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

/// SGL's Gouraud state (a block in its code): the first Gouraud table's number (its VDP1 address
/// over 8, as attributes give it), where the tables are made, the number past the last, the
/// colour of each of the 32 shades (a word each), a shade byte per point, the range of tables
/// made since they were last copied to VDP1 (most and least, 0 and -1 when none), the colour the
/// shades are tinted by (red, green, blue); then the light as the polygon data's frame sees it,
/// and SGL's own shade colours (a grey ramp at the start, slSetGouraudColor's).
const GOURAUD: u32 = 0x0607_12d8;
const GOURAUD_FIRST: u32 = 0;
const GOURAUD_TABLES: u32 = 4;
const GOURAUD_END: u32 = 8;
const GOURAUD_COLOURS: u32 = 12;
const GOURAUD_SHADES: u32 = 16;
const GOURAUD_MOST: u32 = 20;
const GOURAUD_LEAST: u32 = 24;
const GOURAUD_TINT: u32 = 28;
const GOURAUD_LIGHT: u32 = 40;
const GOURAUD_RAMP: u32 = 0x38;
const SHADES: u32 = 32;
const GREY_STEP: u32 = 0x421;
/// The polygon data's normals (one per point, 3 words; 0 when it has none). A polygon's points (4
/// words, after its normal); an attribute's Gouraud table (a word) and its flag (the sort byte's
/// top bit).
const PD_NORMALS: u32 = 0x14;
const NORMAL_SIZE: u32 = 12;
const POLYGON_POINTS: u32 = 12;
const POLYGON_SIZE: u32 = 20;
const ATTR_SORT: u32 = 1;
const ATTR_GOURAUD_ON: u8 = 0x80;
const ATTR_GOURAUD: u32 = 8;
const ATTR_SIZE: u32 = 12;
const SHADE_MOST: i32 = 31;
const SL_PUT_POLYGON: u32 = 0x0606_745c;
const SL_PUSH_MATRIX: u32 = 0x0606_71cc;
const SL_POP_MATRIX: u32 = 0x0606_7200;
const TURN_UNDONE: u32 = 0x0607_20fc;
const TURN_VECTOR: u32 = 0x0607_212c;
const GOURAUD_MAKE: u32 = 0x0607_0ff0;

/// slPutPolygonX: the polygon data at `pd` put (slPutPolygon), then, when it has normals, its
/// Gouraud tables made under the light at `light` (turned into the data's frame by the current
/// matrix's turn undone). What slPopMatrix returns (0 when there are no normals).
pub fn sl_put_polygon_x(cpu: &mut Cpu, pd: u32, light: u32) -> u32 {
    // the SH-2 code keeps them in r8 and r9 (its callees push them)
    cpu.holding(&[(8, pd), (9, light)], |cpu| {
        cpu.call(SL_PUT_POLYGON, &[pd]);
        if cpu.read32(pd + PD_NORMALS) == 0 {
            cpu.leave_t(true);
            return 0;
        }
        cpu.call(SL_PUSH_MATRIX, &[]);
        cpu.call(TURN_UNDONE, &[]);
        cpu.call(TURN_VECTOR, &[light, GOURAUD + GOURAUD_LIGHT]);
        cpu.call(GOURAUD_MAKE, &[pd, GOURAUD + GOURAUD_LIGHT]);
        cpu.call(SL_POP_MATRIX, &[])
    })
}

/// The current matrix's place cleared and its turn transposed (undone, a turn having no scale).
pub fn turn_undone(cpu: &mut Cpu) {
    let m = matrix(cpu);
    for k in [12, 28, 44] {
        cpu.write32(m + k, 0);
    }
    let mut last = (0, 0);
    for (a, b) in [(4, 16), (8, 32), (24, 36)] {
        let (at_a, at_b) = (cpu.read32(m + a), cpu.read32(m + b));
        cpu.write32(m + a, at_b);
        cpu.write32(m + b, at_a);
        last = (at_b, at_a);
    }
    cpu.leave(&[(0, m), (1, last.0), (2, last.1)]);
}

/// `v` (x, y, z) through the current matrix's turn to `out`.
pub fn turn_vector(cpu: &mut Cpu, v: u32, out: u32) {
    let m = matrix(cpu);
    let mut sum = 0;
    for r in 0..3 {
        sum = 0;
        for k in 0..3 {
            let (a, e) = (cpu.read32(v + 4 * k), cpu.read32(m + 16 * r + 4 * k));
            sum = mac(cpu, sum, a, e);
        }
        cpu.write32(out + 4 * r, middle(sum));
    }
    cpu.leave(&[(0, m + MATRIX_SIZE), (1, 0), (2, (sum >> 32) as u32), (3, middle(sum)), (4, v), (5, out + 12)]);
    cpu.leave_mac(sum);
    cpu.leave_t(true);
}

/// The Gouraud tables of the polygon data at `pd`: each point's shade (0-31, a byte) from its
/// normal's dot with the turned light, negated (facing the light is bright; 1 and past the
/// brightest, -1 and past the darkest), then each Gouraud polygon's table (8 bytes: its four
/// points' colours) made, the range of tables made kept. Its loops run once at least, as the SH-2
/// code's do.
pub fn gouraud_make(cpu: &mut Cpu, pd: u32) {
    let light = GOURAUD + GOURAUD_LIGHT;
    let shades = cpu.read32(GOURAUD + GOURAUD_SHADES);
    let (mut normal, mut shade_at) = (cpu.read32(pd + PD_NORMALS), shades);
    let mut n = cpu.read32(pd + PD_POINT_COUNT);
    let mut sum;
    loop {
        sum = 0;
        for k in 0..3 {
            let (a, l) = (cpu.read32(normal + 4 * k), cpu.read32(light + 4 * k));
            sum = mac(cpu, sum, a, l);
        }
        let v = middle(sum).wrapping_neg() as i32;
        let shade = if v >= 0x1_0000 {
            SHADE_MOST
        } else if v <= -0x1_0000 {
            0
        } else {
            ((v >> 12) + 16) & SHADE_MOST
        };
        cpu.write8(shade_at, shade as u8);
        (normal, shade_at) = (normal + NORMAL_SIZE, shade_at + 1);
        n = n.wrapping_sub(1);
        if n == 0 {
            break;
        }
    }
    let colours = cpu.read32(GOURAUD + GOURAUD_COLOURS);
    let (mut most, mut least) = (cpu.read32(GOURAUD + GOURAUD_MOST), cpu.read32(GOURAUD + GOURAUD_LEAST));
    let (first, tables) = (cpu.read32(GOURAUD + GOURAUD_FIRST), cpu.read32(GOURAUD + GOURAUD_TABLES));
    let (mut polygon, mut attr) = (cpu.read32(pd + PD_POLYGONS), cpu.read32(pd + PD_ATTRS));
    let mut n = cpu.read32(pd + PD_POLYGON_COUNT);
    loop {
        if cpu.read8(attr + ATTR_SORT) & ATTR_GOURAUD_ON != 0 {
            let table = (cpu.read16(attr + ATTR_GOURAUD) as u32).wrapping_sub(first);
            least = least.min(table);
            most = most.max(table);
            let at = (table << 3).wrapping_add(tables);
            for k in 0..4 {
                let point = cpu.read16(polygon + POLYGON_POINTS + 2 * k) as i16 as i32 as u32;
                let shade = cpu.read8(shades.wrapping_add(point)) as i8 as i32 as u32;
                let colour = cpu.read16(colours.wrapping_add(shade << 1));
                cpu.write16(at.wrapping_add(2 * k), colour);
            }
        }
        (polygon, attr) = (polygon + POLYGON_SIZE, attr + ATTR_SIZE);
        n = n.wrapping_sub(1);
        if n == 0 {
            break;
        }
    }
    cpu.write32(GOURAUD + GOURAUD_MOST, most);
    cpu.write32(GOURAUD + GOURAUD_LEAST, least);
    cpu.leave(&[(0, GOURAUD), (1, 0), (2, polygon), (3, attr), (4, shades), (5, colours), (6, most), (7, least)]);
    cpu.leave_mac(sum);
    cpu.leave_t(true);
}

/// slInitGouraud: where the Gouraud tables are made (`tables`), how many (`count`), the first's
/// number (`first`) and the shades' buffer; none made yet, no tint, the shades' colours the grey
/// ramp (shade k red, green and blue k).
pub fn sl_init_gouraud(cpu: &mut Cpu, tables: u32, count: u32, first: u32, shades: u32) {
    let end = count.wrapping_add(first);
    cpu.write32(GOURAUD + GOURAUD_TABLES, tables);
    cpu.write32(GOURAUD + GOURAUD_FIRST, first);
    cpu.write32(GOURAUD + GOURAUD_END, end);
    cpu.write32(GOURAUD + GOURAUD_SHADES, shades);
    for k in 0..3 {
        cpu.write32(GOURAUD + GOURAUD_TINT + 4 * k, 0);
    }
    cpu.write32(GOURAUD + GOURAUD_MOST, 0);
    cpu.write32(GOURAUD + GOURAUD_LEAST, u32::MAX);
    let ramp = GOURAUD + GOURAUD_RAMP;
    cpu.write32(GOURAUD + GOURAUD_COLOURS, ramp);
    for k in 0..SHADES {
        cpu.write16(ramp + 2 * k, (k * GREY_STEP) as u16);
    }
    cpu.leave(&[(0, ramp + 2 * SHADES), (1, 0), (2, SHADES * GREY_STEP), (3, GREY_STEP), (5, end)]);
    cpu.leave_t(true);
}

/// slLight: the light's direction (`v`, x, y, z) kept negated and sent to the slave so.
const LIGHT: u32 = 0x4c;
const LIGHT_COMMAND: u32 = 36;
const LIGHT_COMMAND_SIZE: u32 = 16;

pub fn sl_light(cpu: &mut Cpu, v: u32) {
    let w = cpu.gbr();
    let c = cpu.read32(w + COMMANDS);
    let d = [0, 1, 2].map(|k| cpu.read32(v + 4 * k));
    for (k, e) in (0..).zip(d) {
        cpu.write32(c + 4 + 4 * k, e.wrapping_neg());
        cpu.write32(w + LIGHT + 4 * k, e.wrapping_neg());
    }
    cpu.write32(c, LIGHT_COMMAND);
    let next = c + LIGHT_COMMAND_SIZE;
    cpu.write32(w + COMMANDS, next);
    cpu.write16(SLAVE_SIGNAL, next as u16);
    cpu.leave(&[(0, next), (1, SLAVE_SIGNAL), (2, d[1]), (3, d[2]), (4, LIGHT_COMMAND), (7, next)]);
}

/// SGL's VDP2 image (VDP2's register N at 0x060ffcc0 + N, GBR + 0xc0 + N, from GBR here): RAM
/// control (RAMCTL), the screens' shown and transparent bits (BGON, its high byte transparency),
/// the character controls (CHCTLA: NBG1's byte, then NBG0's; CHCTLB: RBG0's, then NBG3's and
/// NBG2's nibbles), the pattern name controls (from the image: PNCN0-PNCN3, PNCR), the plane
/// sizes (PLSZ, its low byte NBG0's to NBG3's two bits), the map offsets (MPOFN: NBG0-NBG3 a
/// nibble each, MPOFR), the maps (a word for two planes: MPABN0 on, RBG0's from MPABRA), the
/// zoom control (ZMCTL: NBG1's byte, then NBG0's) and the back screen's table (BKTAU).
const VDP2_IMAGE: u32 = 0x060f_fcc0;
const IMAGE: u32 = 0xc0;
const RAMCTL: u32 = 0xce;
const BGON_HIGH: u32 = 0xe0;
const CHCTLA: u32 = 0xe8;
const CHCTLB: u32 = 0xea;
const PNCN0: u32 = 0x30;
const PNCN1: u32 = 0x32;
const PNCN2: u32 = 0x34;
const PNCN3: u32 = 0x36;
const PNCR: u32 = 0x38;
const PLSZ_LOW: u32 = 0xfb;
const MPOFN: u32 = 0xfc;
const MPOFR_LOW: u32 = 0xff;
const MPABN0: u32 = 0x100;
const MPABN1: u32 = 0x104;
const MPABN2: u32 = 0x108;
const MPABRA: u32 = 0x110;
const ZMCTL_IMAGE: u32 = 0x060f_fd58;
const BKTAU: u32 = 0x16c;
/// Each screen's cells' address as its page setting keeps it (NBG0 4, NBG1 8, NBG2 12, NBG3 16,
/// RBG0 20 on), and its map's first plane as its map setting does (from GBR: NBG0's on).
const SCREEN_CELLS: u32 = 0x060f_fde0;
const SCREEN_MAPS: u32 = 0x1fc;
const RBG0_MAP: u32 = 0x20c;

/// slCharNbg0/1/2/3, slCharRbg0: the screen's colour count and character size (`kind`, `size`:
/// 2x2 cells 1) in its character control; then the supplementary character number in its pattern
/// name control from its cells' address as its page setting last kept it (1x1 cells: the
/// address's bits 15-19; 2x2: bits 5-6). NBG3's tests the wrong bit (its size, shifted): always
/// 1x1.
fn char_names(cpu: &mut Cpu, c: u32, screen: u32, pnc: u32) {
    let name_at = VDP2_IMAGE + pnc;
    let cells = cpu.read32(SCREEN_CELLS + screen);
    let low = cpu.read8(name_at + 1) as i8 as i32 as u32;
    let (bits, number) = if c & 1 == 0 {
        cpu.leave_t(cells >> 31 != 0);
        let bits = (cells << 1) >> 16;
        (bits, bits & 31)
    } else {
        cpu.leave_t(cells >> 4 & 1 != 0);
        let bits = cells >> 5;
        (bits, bits & 3)
    };
    let v = (low & if c & 1 == 0 { 0xe0 } else { 0xfc }) | number;
    cpu.write8(name_at + 1, v as u8);
    cpu.leave(&[(0, v), (1, number), (2, name_at), (3, bits)]);
}

pub fn sl_char_nbg0(cpu: &mut Cpu, kind: u32, size: u32) {
    let w = cpu.gbr();
    let c = kind | size;
    cpu.write8(w + CHCTLA + 1, c as u8);
    cpu.leave(&[(4, c)]);
    char_names(cpu, c, 4, PNCN0);
}

pub fn sl_char_nbg1(cpu: &mut Cpu, kind: u32, size: u32) {
    let w = cpu.gbr();
    let c = kind | size;
    cpu.write8(w + CHCTLA, c as u8);
    cpu.leave(&[(4, c)]);
    char_names(cpu, c, 8, PNCN1);
}

pub fn sl_char_nbg2(cpu: &mut Cpu, kind: u32, size: u32) {
    let w = cpu.gbr();
    let b = cpu.read8(w + CHCTLB + 1) as i8 as i32 as u32;
    let c = (b & !0xf) | kind >> 3 | size;
    cpu.write8(w + CHCTLB + 1, c as u8);
    cpu.leave(&[(4, kind >> 3)]);
    char_names(cpu, c, 12, PNCN2);
}

pub fn sl_char_nbg3(cpu: &mut Cpu, kind: u32, size: u32) {
    let w = cpu.gbr();
    let b = cpu.read8(w + CHCTLB + 1) as i8 as i32 as u32;
    let c = (b & 0xf) | kind << 1 | size << 4;
    cpu.write8(w + CHCTLB + 1, c as u8);
    cpu.leave(&[(4, kind << 1 | size << 4), (5, size << 4)]);
    char_names(cpu, size << 4, 16, PNCN3);
}

pub fn sl_char_rbg0(cpu: &mut Cpu, kind: u32, size: u32) {
    let w = cpu.gbr();
    let c = kind | size;
    cpu.write8(w + CHCTLB, c as u8);
    cpu.leave(&[(4, c)]);
    char_names(cpu, c, 20, PNCR);
}

/// slPageNbg0/1/2/3, slPageRbg0: the screen's cells' address (`cells`) kept, and its pattern name
/// control made from it, its colour palette's address (`palette`: bank bits 9-11, 10-12 in colour
/// RAM's mode 2) and the pattern names' kind (`kind`): the supplementary character number as
/// slChar's (`c`, the character control's bits, says 2x2), the palette's bits 5-7. RBG0's VRAM
/// bank given to its cells first.
fn page_names(cpu: &mut Cpu, c: u32, screen: u32, pnc: u32, cells: u32, palette: u32, kind: u32) {
    let w = cpu.gbr();
    cpu.write32(SCREEN_CELLS + screen, cells);
    let (mut r3, mut kind, mut mask) = (c, kind, 31);
    if c & 1 != 0 {
        r3 = cells >> 5;
        kind |= r3 & 3;
        mask = 28;
    }
    let bits = (cells << 1) >> 16;
    let mode = (cpu.read16(w + RAMCTL) as i16 as i32 as u32) >> 8;
    let shift = if mode & 0x20 != 0 { 10 } else { 9 };
    let bank = ((palette >> shift) & 7) << 5;
    let v = (bits & mask) | kind | bank;
    cpu.write16(VDP2_IMAGE + pnc, v as u16);
    cpu.leave(&[(0, VDP2_IMAGE), (1, v), (2, pnc), (3, r3), (4, bits), (5, bank), (6, kind)]);
    cpu.leave_t(false);
}

pub fn sl_page_nbg0(cpu: &mut Cpu, cells: u32, palette: u32, kind: u32) {
    let w = cpu.gbr();
    let c = cpu.read8(w + CHCTLA + 1) as i8 as i32 as u32;
    page_names(cpu, c, 4, PNCN0, cells, palette, kind);
}

pub fn sl_page_nbg1(cpu: &mut Cpu, cells: u32, palette: u32, kind: u32) {
    let w = cpu.gbr();
    let c = cpu.read8(w + CHCTLA) as i8 as i32 as u32;
    page_names(cpu, c, 8, PNCN1, cells, palette, kind);
}

pub fn sl_page_nbg2(cpu: &mut Cpu, cells: u32, palette: u32, kind: u32) {
    let w = cpu.gbr();
    let c = cpu.read8(w + CHCTLB + 1) as i8 as i32 as u32;
    page_names(cpu, c, 12, PNCN2, cells, palette, kind);
}

pub fn sl_page_nbg3(cpu: &mut Cpu, cells: u32, palette: u32, kind: u32) {
    let w = cpu.gbr();
    let c = (cpu.read8(w + CHCTLB + 1) as i8 as i32 as u32) >> 4;
    page_names(cpu, c, 16, PNCN3, cells, palette, kind);
}

const ROTATION_BANK: u32 = 0x0607_1cde;
const BANK_CELLS: u32 = 3;
const BANK_NAMES: u32 = 2;

pub fn sl_page_rbg0(cpu: &mut Cpu, cells: u32, palette: u32, kind: u32) {
    // the SH-2 code keeps the palette on the stack meanwhile
    cpu.push(&[palette]);
    cpu.call(ROTATION_BANK, &[cells, BANK_CELLS]);
    cpu.pop(4);
    let w = cpu.gbr();
    let c = cpu.read8(w + CHCTLB) as i8 as i32 as u32;
    page_names(cpu, c, 20, PNCR, cells, palette, kind);
}

/// The VRAM bank holding `at` (A0, A1, B0, B1: its bits 17-18) given to RBG0 for `use` (RAMCTL's
/// bank bits: 1 coefficients, 2 pattern names, 3 cells), any bank that had that use losing it.
pub fn rotation_bank(cpu: &mut Cpu, at: u32, use_: u32) {
    let w = cpu.gbr();
    let mut r = cpu.read16(w + RAMCTL) as i16 as i32 as u32;
    for bank in 0..4 {
        let mask = 3 << (2 * bank);
        if r & mask == use_ << (2 * bank) {
            r &= !mask;
        }
    }
    let swapped = at.rotate_left(16);
    let shift = if swapped & 4 != 0 { 4 } else { 0 } + if swapped & 2 != 0 { 2 } else { 0 };
    let mask = 3 << shift;
    let v = (r & !mask) | use_ << shift;
    cpu.write16(w + RAMCTL, v as u16);
    cpu.leave(&[(0, v), (1, r), (2, mask), (3, 0), (5, use_ << shift)]);
    cpu.leave_t(swapped & 2 == 0);
}

/// slMapNbg0/1/2: the screen's four planes' maps (`a`-`d`, VRAM addresses) set: `a` kept, the
/// map offset (a nibble's three bits: `a` over 64 pages) and each plane's page (its address over
/// a page, 6 bits). A page's size as a shift: 2 KB, doubled with 2-word pattern names (the
/// pattern name control's top bit clear), four times with 1x1 cells (the character size clear).
fn page_shift(cpu: &mut Cpu, pnc: u32, chctl: u32, high: bool) -> u32 {
    let w = cpu.gbr();
    let mut s = 11;
    if cpu.read16(w + IMAGE + pnc) & 0x8000 == 0 {
        s += 1;
    }
    let c = cpu.read16(w + chctl) as u32;
    if (if high { c >> 8 } else { c }) & 1 == 0 {
        s += 2;
    }
    s
}

fn map_set(cpu: &mut Cpu, s: u32, mpof: u32, high: bool, mpab: u32, planes: [u32; 4]) {
    let w = cpu.gbr();
    let a = planes[0];
    let offset = ((a >> (s + 6)) & 7) << if high { 4 } else { 0 };
    let m = cpu.read8(w + mpof) as i8 as i32 as u32;
    let v = (m & if high { 0x0f } else { 0xf0 }) | offset;
    cpu.write8(w + mpof, v as u8);
    let [a, b, c, d] = planes.map(|p| (p >> s) & 63);
    let (ab, cd) = (b << 8 | a, d << 8 | c);
    cpu.write16(w + mpab, ab as u16);
    cpu.write16(w + mpab + 2, cd as u16);
    cpu.leave(&[(0, cd), (1, 63), (2, offset), (3, s), (4, ab), (5, b << 8), (6, cd), (7, d << 8)]);
    cpu.leave_t(s & 1 == 0);
}

pub fn sl_map_nbg0(cpu: &mut Cpu, a: u32, b: u32, c: u32, d: u32) {
    let w = cpu.gbr();
    cpu.write32(w + SCREEN_MAPS, a);
    let s = page_shift(cpu, PNCN0, CHCTLA, false);
    map_set(cpu, s, MPOFN + 1, false, MPABN0, [a, b, c, d]);
}

pub fn sl_map_nbg1(cpu: &mut Cpu, a: u32, b: u32, c: u32, d: u32) {
    let w = cpu.gbr();
    cpu.write32(w + SCREEN_MAPS + 4, a);
    let s = page_shift(cpu, PNCN1, CHCTLA, true);
    map_set(cpu, s, MPOFN + 1, true, MPABN1, [a, b, c, d]);
}

pub fn sl_map_nbg2(cpu: &mut Cpu, a: u32, b: u32, c: u32, d: u32) {
    let w = cpu.gbr();
    cpu.write32(w + SCREEN_MAPS + 8, a);
    let s = page_shift(cpu, PNCN2, CHCTLB, false);
    map_set(cpu, s, MPOFN, false, MPABN2, [a, b, c, d]);
}

/// slMapRbg0: RBG0's (its parameter A's) sixteen planes all on the map at `a`, its VRAM bank
/// given to the pattern names.
pub fn sl_map_rbg0(cpu: &mut Cpu, a: u32) {
    let w = cpu.gbr();
    cpu.write32(w + RBG0_MAP, a);
    cpu.call(ROTATION_BANK, &[a, BANK_NAMES]);
    let s = page_shift(cpu, PNCR, CHCTLB, true);
    let offset = (a >> (s + 6)) & 7;
    let m = cpu.read8(w + MPOFR_LOW) as i8 as i32 as u32;
    cpu.write8(w + MPOFR_LOW, ((m & 0xf0) | offset) as u8);
    let page = (a >> s) & 63;
    let v = page << 8 | page;
    for k in 0..8 {
        cpu.write16(w + MPABRA + 2 * k, v as u16);
    }
    cpu.leave(&[(0, v), (1, 63), (2, offset), (3, s), (4, page)]);
    cpu.leave_t(s & 1 == 0);
}

/// slPlaneNbg0/1/2: the screen's plane size (`size`: 1x1, 2x1, 2x2 pages) in PLSZ.
fn plane_size(cpu: &mut Cpu, keep: u32, v: u32) {
    let w = cpu.gbr();
    let p = (cpu.read8(w + PLSZ_LOW) as i8 as i32 as u32 & keep) | v;
    cpu.write8(w + PLSZ_LOW, p as u8);
    cpu.leave(&[(0, p)]);
}

pub fn sl_plane_nbg0(cpu: &mut Cpu, size: u32) {
    plane_size(cpu, 0xfc, size);
}

pub fn sl_plane_nbg1(cpu: &mut Cpu, size: u32) {
    plane_size(cpu, 0xf3, size << 2);
    cpu.leave(&[(4, size << 2)]);
}

pub fn sl_plane_nbg2(cpu: &mut Cpu, size: u32) {
    plane_size(cpu, 0xcf, size << 4);
    cpu.leave(&[(4, size << 4)]);
}

/// slZoomMode: the screen's (`screen`: NBG1 0, NBG0 1) zoom control byte set to `mode`.
pub fn sl_zoom_mode(cpu: &mut Cpu, screen: u32, mode: u32) {
    cpu.write8(ZMCTL_IMAGE.wrapping_add(screen), mode as u8);
    cpu.leave(&[(0, ZMCTL_IMAGE)]);
}

/// slBack1ColSet: the back screen one colour (`colour`) at `at` in VRAM, its table there.
pub fn sl_back1_col_set(cpu: &mut Cpu, at: u32, colour: u32) {
    let w = cpu.gbr();
    cpu.write16(at, colour as u16);
    cpu.write32(w + BKTAU, at >> 1);
    cpu.leave(&[(0, at >> 1)]);
    cpu.leave_t(at & 1 != 0);
}

/// VRAM A and B each split in two banks or not (RAMCTL's VRAMD and VRBMD bits, `split`).
pub fn vram_split(cpu: &mut Cpu, split: u32) {
    let w = cpu.gbr();
    let v = (cpu.read8(w + RAMCTL) as i8 as i32 as u32 & 0xfc) | split;
    cpu.write8(w + RAMCTL, v as u8);
    cpu.leave(&[(0, v)]);
}

/// slScrTransparent: the screens whose transparent dots are drawn (BGON's high byte, `bits`).
pub fn sl_scr_transparent(cpu: &mut Cpu, bits: u32) {
    let w = cpu.gbr();
    cpu.write8(w + BGON_HIGH, bits as u8);
    cpu.leave(&[(0, bits)]);
}

/// slZdspLevel: the near limit's level (`level`, 1-7: the window's depth over 2 to it, WIN_FLAG)
/// set and sent to the slave (command 52). Whether it was in range.
const ZDSP_COMMAND: u32 = 52;

pub fn sl_zdsp_level(cpu: &mut Cpu, level: u32) -> u32 {
    cpu.leave(&[(1, 8)]);
    if level as i32 <= 0 || level >= 8 {
        cpu.leave_t(level as i32 > 0);
        return 0;
    }
    let w = cpu.gbr();
    cpu.write8(w + WIN_FLAG, level as u8);
    let c = cpu.read32(w + COMMANDS);
    cpu.write32(c, ZDSP_COMMAND);
    cpu.write32(c + 4, level);
    let next = c + 8;
    cpu.write32(w + COMMANDS, next);
    cpu.write16(SLAVE_SIGNAL, next as u16);
    cpu.leave(&[(1, SLAVE_SIGNAL)]);
    cpu.leave_t(false);
    1
}

/// Whether DMA channel 0 is copying (CHCR0's enable bit set, its end bit clear).
pub fn dma0_busy(cpu: &mut Cpu) -> u32 {
    let busy = cpu.read32(CHCR0) & 3 == 1;
    cpu.leave(&[(1, CHCR0 - 12)]);
    cpu.leave_t(busy);
    busy as u32
}

/// slSetGouraudColor: SGL's shade colours made from `rgb` (5 bits each of red, green, blue) and
/// used: each part of shade k is k less what the part lacks of 31, but not below the tint's.
pub fn sl_set_gouraud_color(cpu: &mut Cpu, rgb: u32) {
    let lack = [0, 5, 10].map(|s| 31 - ((rgb >> s) & 31));
    let ramp = GOURAUD + GOURAUD_RAMP;
    cpu.write32(GOURAUD + GOURAUD_COLOURS, ramp);
    let tint = [0, 1, 2].map(|k| cpu.read32(GOURAUD + GOURAUD_TINT + 4 * k));
    for k in 0..SHADES {
        let mut v = 0;
        for (c, s) in [0, 5, 10].into_iter().enumerate() {
            let part = (k.wrapping_sub(lack[c]) as i32).max(tint[c] as i32) as u32;
            v |= part << s;
        }
        cpu.write16(ramp + 2 * k, v as u16);
    }
    cpu.leave(&[(0, SHADES), (1, lack[0]), (2, lack[1]), (3, lack[2]), (4, ramp + 2 * SHADES)]);
    cpu.leave(&[(5, tint[0]), (6, tint[1]), (7, tint[2])]);
    cpu.leave_t(true);
}

/// slPerspective: the view's angle (`angle`): the window's depth (half the screen's width over
/// the tangent of half the angle, on the divide unit) kept and sent to the slave (command 12),
/// and the rotation parameter tables' Pz and Cz (both tables: its whole part, halved when the
/// screen mode's bit 3 says so) set from it.
const SCREEN_WIDTH: u32 = 0x80;
const SCREEN_MODE: u32 = 0xb0;
const PERSPECTIVE_COMMAND: u32 = 12;
/// SGL's rotation parameter tables (A, then B 104 on: VDP2's table, then its own fields): A's Pz.
const ROTATION_PZ: u32 = 0x060f_fe54;
const ROTATION_TABLE: u32 = 104;
const PZ_TO_CZ: u32 = 8;
const SL_TAN: u32 = 0x0606_eadc;

pub fn sl_perspective(cpu: &mut Cpu, angle: u32) {
    let w = cpu.gbr();
    let t = cpu.call(SL_TAN, &[(angle as i32 >> 1) as u32]);
    let width = cpu.read16(w + SCREEN_WIDTH) as u32;
    let half = (width << 16) >> 1;
    cpu.write32(DVSR, if t as i32 >= 0 { t } else { t.wrapping_neg() });
    cpu.write32(DVDNTH, (half >> 16) as u16 as i16 as i32 as u32);
    cpu.write32(DVDNTL, half << 16);
    let c = cpu.read32(w + COMMANDS);
    cpu.write32(c, PERSPECTIVE_COMMAND);
    let depth = cpu.read32(DVDNTL_AGAIN);
    cpu.write32(c + 4, depth);
    cpu.write32(w + WIN_DEPTH, depth);
    let next = c + 8;
    cpu.write32(w + COMMANDS, next);
    cpu.write16(SLAVE_SIGNAL, next as u16);
    let halved = cpu.read8(w + SCREEN_MODE) & 8 != 0;
    let d = if halved { depth >> 1 } else { depth };
    let v = d.rotate_left(16);
    for at in [0, PZ_TO_CZ, ROTATION_TABLE, ROTATION_TABLE + PZ_TO_CZ] {
        cpu.write16(ROTATION_PZ + at, v as u16);
    }
    cpu.leave(&[(0, v), (1, d), (2, ROTATION_PZ + ROTATION_TABLE), (7, c)]);
    cpu.leave_t(!halved);
}

/// The rotation scroll's parameters from the current matrix (the view's inverse) into the table
/// slCurRpara made current ([GBR+0x1e0]: VDP2's rotation parameter table, then SGL's coefficient
/// table's address and a mode word): A-F the first three columns of its first two rows, Mx and
/// My the rows' places less the centre's (Px, Py), and the coefficients, minus the third row's
/// place (`num`) over the depth across the screen (the third row's dot with minus the centre,
/// `z`, moving by its x and y, `dx` and `dy`). Either a table of 512 lines (16-bit coefficients
/// with the mode's bit 1, 32-bit without; the lines spread evenly over the depths the screen's
/// corners reach, a line behind the eye marked off; the quotients on the divide unit), or with
/// the mode's bit 6 one coefficient, the parameters scaled by a power of 2 to keep them in range.
/// The table's busy byte cleared meanwhile. (SGL's code has a fourth way that nothing reaches.)
const ROTATION_PARAMS: u32 = 0x1e0;
const ROTATION_BUSY: u32 = 0xcc;
const RP_A: u32 = 0x1c;
const RP_PX: u32 = 0x34;
const RP_MX: u32 = 0x44;
const RP_KAST: u32 = 0x54;
const RP_DKAST: u32 = 0x58;
const RP_DKAX: u32 = 0x5c;
const RP_TABLE: u32 = 0x60;
const RP_MODE: u32 = 0x64;
const RP_ONE_COEFFICIENT: u16 = 0x40;
const RP_SHORT_COEFFICIENTS: u16 = 2;
const SCREEN_HEIGHT: u32 = 0x82;
const COEFFICIENT_LINES: u32 = 512;
const LINES_SPREAD: u32 = 0x01ff_0000;
const BEHIND: u32 = 0x8000_0000;
const POWERS_OF_TWO: u32 = 0x0606_e970;
const MATRIX_AF: [u32; 6] = [0, 4, 8, 16, 20, 24];

fn quotient(cpu: &mut Cpu, by: u32, v: u32) -> u32 {
    divide(cpu, by, v);
    cpu.read32(DVDNTL_AGAIN)
}

pub fn rotation_parameters(cpu: &mut Cpu) -> u32 {
    let w = cpu.gbr();
    cpu.write8(w + ROTATION_BUSY, 0);
    // the SH-2 code pushes GBR below its other pushes (it addresses the table from it meanwhile),
    // and minus Py then minus Pz below that for MAC.L
    let sp = cpu.stack();
    cpu.write32(sp - 4, w);
    let m = matrix(cpu);
    let p = cpu.read32(w + ROTATION_PARAMS);
    for (k, at) in (0..).zip(MATRIX_AF) {
        let v = cpu.read32(m + at);
        cpu.write32(p + RP_A + 4 * k, v);
    }
    let [px, py, pz] = [0, 1, 2].map(|k| (cpu.read16(p + RP_PX + 2 * k) as u32) << 16);
    let mx = cpu.read32(m + 12).wrapping_sub(px);
    cpu.write32(p + RP_MX, mx);
    let my = cpu.read32(m + 28).wrapping_sub(py);
    cpu.write32(p + RP_MX + 4, my);
    let mut num = cpu.read32(m + 44).wrapping_neg();
    let (mut dx, mut dy, m22) = (cpu.read32(m + 32), cpu.read32(m + 36), cpu.read32(m + 40));
    let mut sum = product(dx, px.wrapping_neg());
    sum = mac(cpu, sum, py.wrapping_neg(), dy);
    sum = mac(cpu, sum, pz.wrapping_neg(), m22);
    cpu.write32(sp - 8, pz.wrapping_neg());
    let zst = middle(sum);
    let table = cpu.read32(p + RP_TABLE);
    let mode = cpu.read16(p + RP_MODE);
    let mut z = zst;
    if (num as i32) < 0 {
        (num, z, dx, dy) = (num.wrapping_neg(), z.wrapping_neg(), dx.wrapping_neg(), dy.wrapping_neg());
    }
    let screen = cpu.read8(w + SCREEN_MODE);
    // the screen's width or height less 1 (halved by the screen mode's bit)
    let size = |cpu: &mut Cpu, at: u32, bit: u8| {
        let s = cpu.read16(w + at) as i16 as i32 as u32;
        (if screen & bit != 0 { s >> 1 } else { s }).wrapping_sub(1)
    };
    if mode & RP_ONE_COEFFICIENT != 0 {
        let mut k = quotient(cpu, num, 0x0080_0000);
        let x = size(cpu, SCREEN_WIDTH, 8).wrapping_mul(dx);
        let y = size(cpu, SCREEN_HEIGHT, 8).wrapping_mul(dy);
        let (mut ahead, mut behind) = (0u32, 0u32);
        for v in [x, y, z] {
            if v as i32 >= 0 {
                ahead = ahead.wrapping_add(v);
            } else {
                behind = behind.wrapping_sub(v);
            }
        }
        let most = ahead.max(behind);
        let mut big = 0;
        for k in 0..6 {
            big |= (cpu.read32(p + RP_A + 4 * k) as i32).unsigned_abs();
        }
        let bits = |v: u32, past: i32| past - v.leading_zeros() as i32;
        let shift = (bits(most, 16) + bits(k, 16)).max(bits(big, 26)) - 13;
        if shift != 0 {
            let s = cpu.read32(POWERS_OF_TWO.wrapping_add((15 - shift) as u32 * 4));
            k = middle(product(s, k));
            for (n, at) in (0..).zip(MATRIX_AF) {
                let v = cpu.read32(m + at);
                cpu.write32(p + RP_A + 4 * n, middle(product(s, v)));
            }
        }
        let kast = middle(product(k, z)).wrapping_add(0x4000_0000);
        cpu.write32(p + RP_KAST, ((table >> 2) << 16).wrapping_add(kast));
        cpu.write32(p + RP_DKAX, middle(product(k, dx)));
        let last = product(k, dy);
        cpu.write32(p + RP_DKAST, middle(last));
        cpu.leave(&[(1, (last >> 32) as u32), (2, kast), (3, most), (4, p + RP_A + 24)]);
        cpu.leave_mac(last);
        cpu.leave_t(shift == 0);
    } else {
        let short = mode & RP_SHORT_COEFFICIENTS != 0;
        let (x, y) = if short {
            let x = middle(product(size(cpu, SCREEN_WIDTH, 8) << 16, dx));
            sum = product(size(cpu, SCREEN_HEIGHT, 16) << 16, dy);
            (x, middle(sum))
        } else {
            let x = size(cpu, SCREEN_WIDTH, 8).wrapping_mul(dx);
            sum = product(size(cpu, SCREEN_HEIGHT, 8), dy);
            (x, sum as u32)
        };
        let (mut ahead, mut behind) = (0u32, 0u32);
        for v in [x, y] {
            if v as i32 >= 0 {
                ahead = ahead.wrapping_add(v);
            } else {
                behind = behind.wrapping_add(v);
            }
        }
        let step = quotient(cpu, LINES_SPREAD, ahead.wrapping_sub(behind));
        let first = quotient(cpu, step, behind.wrapping_neg());
        let base = if short { (table << 16) >> 2 } else { (table >> 2) << 16 };
        cpu.write32(p + RP_KAST, base.wrapping_add(first));
        let (mut z, mut at) = (z.wrapping_add(behind), table);
        for _ in 0..COEFFICIENT_LINES {
            let q = if z as i32 >= 0 { Some(quotient(cpu, z, num)) } else { None };
            if short {
                cpu.write16(at, q.map_or(BEHIND >> 16, |q| (q >> 6) & 0x3fff) as u16);
                at += 2;
            } else {
                cpu.write32(at, q.map_or(BEHIND, |q| q & 0x7f_ffff));
                at += 4;
            }
            z = z.wrapping_add(step);
        }
        let dkax = quotient(cpu, step, dx);
        cpu.write32(p + RP_DKAX, dkax);
        let dkast = quotient(cpu, step, dy);
        cpu.write32(p + RP_DKAST, dkast);
        cpu.leave(&[(1, dy << 16), (2, (dy as i32 >> 16) as u32), (3, DVSR), (4, at)]);
        cpu.leave_mac(sum);
        cpu.leave_t(true);
    }
    cpu.leave(&[(5, zst), (6, m), (7, num)]);
    cpu.write8(w + ROTATION_BUSY, 1);
    1
}

/// The TV mode (`mode`, 0-31; SGL's table of TVMD's low byte by it, and of the rotation tables'
/// Pz and Cz), unless its hi-res bit (bit 2) differs from the screen mode's: whether it was set.
/// Mode bit 4 clears the screen mode's bit 7.
const TV_MODES: u32 = 0x0603_5820;
const TV_DEPTHS: u32 = 0x0603_57a0;
const TVMD_LOW: u32 = 0xc1;

pub fn tv_mode(cpu: &mut Cpu, mode: u32) -> u32 {
    let w = cpu.gbr();
    let mode = mode & 31;
    let screen = cpu.read8(w + SCREEN_MODE) as i8 as i32 as u32;
    let change = mode ^ screen;
    cpu.leave(&[(4, mode), (6, change)]);
    if change & 4 != 0 {
        cpu.leave(&[(1, 31), (2, 16)]);
        cpu.leave_t(false);
        return 0;
    }
    if mode & 16 != 0 {
        cpu.write8(w + SCREEN_MODE, (screen & 0x7f) as u8);
    }
    let tvmd = cpu.read8(TV_MODES + mode);
    cpu.write8(w + TVMD_LOW, tvmd);
    let depth = cpu.read32(TV_DEPTHS + 4 * mode).rotate_left(16);
    for at in [0, PZ_TO_CZ, ROTATION_TABLE, ROTATION_TABLE + PZ_TO_CZ] {
        cpu.write16(ROTATION_PZ + at, depth as u16);
    }
    cpu.leave(&[(1, depth.rotate_left(16)), (2, ROTATION_PZ + ROTATION_TABLE)]);
    cpu.leave_t(mode & 16 == 0);
    1
}

/// Settings SGL sends the slave as they are set (each kept in the work area too; their meanings
/// not known yet): command 60's (0x378), command 68's (0x388).
fn slave_setting(cpu: &mut Cpu, at: u32, command: u32, v: u32) {
    let w = cpu.gbr();
    cpu.write32(w + at, v);
    let c = cpu.read32(w + COMMANDS);
    cpu.write32(c + 4, v);
    cpu.write32(c, command);
    let next = c + 8;
    cpu.write32(w + COMMANDS, next);
    cpu.write16(SLAVE_SIGNAL, next as u16);
    cpu.leave(&[(0, next), (1, SLAVE_SIGNAL)]);
}

pub fn slave_setting_60(cpu: &mut Cpu, v: u32) {
    slave_setting(cpu, 0x378, 60, v);
}

pub fn slave_setting_68(cpu: &mut Cpu, v: u32) {
    slave_setting(cpu, 0x388, 68, v);
}

/// The screen's size for the current window (`width` at most 320, `height` at most 512): kept
/// (WIN_SCREEN: the height, then the width), twice it added to the window's size (WIN_SIZE), and
/// the window's span (its right less its left, each word) with it as the screen's far corner.
const SCREEN_MOST: (u32, u32) = (0x140, 0x200);
const WIN_SIZE_SCREEN: u32 = 0x8c;
const WIN_SCREEN_FAR: u32 = 0x9c;

pub fn screen_size(cpu: &mut Cpu, width: u32, height: u32) {
    let w = cpu.gbr();
    let cur = cpu.read8(w + CUR_WINDOW) as i8 as i32 as u32;
    let s = WINDOWS.wrapping_add(cur << 2);
    let width = if SCREEN_MOST.0 as i32 >= width as i32 { width } else { SCREEN_MOST.0 };
    let height = if SCREEN_MOST.1 as i32 >= height as i32 { height } else { SCREEN_MOST.1 };
    let (right, left) = (cpu.read32(s + W_RIGHT), cpu.read32(s + W_LEFT));
    let size = (height << 16) | (width & 0xffff);
    cpu.write32(w + WIN_SCREEN, size);
    let twice = size.rotate_left(16).wrapping_mul(2);
    let v = cpu.read32(w + WIN_SIZE).wrapping_add(twice);
    cpu.write32(w + WIN_SIZE_SCREEN, v);
    let low = right.wrapping_sub(left).wrapping_add(height) << 16;
    let high = right.rotate_left(16).wrapping_sub(left.rotate_left(16)).wrapping_add(width);
    let far = (high << 16) | (low >> 16);
    cpu.write32(w + WIN_SCREEN_FAR, far);
    cpu.write32(w + WIN_SCREEN_X, far);
    cpu.leave(&[(0, far), (1, twice >> 1), (2, SCREEN_MOST.0), (3, high), (4, width), (5, height)]);
    cpu.leave(&[(6, left.rotate_left(16))]);
    cpu.leave_t(false);
}

/// The sprites' colour mode (SPCTL's SPCLMD, bit 5: `mode`).
const SPCTL: u32 = 0x1a0;

pub fn sprite_colour_mode(cpu: &mut Cpu, mode: u32) {
    let w = cpu.gbr();
    let v = (cpu.read16(w + SPCTL) as i16 as i32 as u32 & !0x20) | mode << 5;
    cpu.write16(w + SPCTL, v as u16);
    cpu.leave(&[(0, v), (1, !0x20), (4, mode << 5)]);
}

/// slTVOn, slTVOff: the display on or off (TVMD's DISP bit; on at the next VBlank, off at once
/// too).
const TVMD: u32 = 0xc0;
const VDP2_TVMD: u32 = 0x25f8_0000;
const DISP: u32 = 0xffff_8000;

pub fn sl_tv_on(cpu: &mut Cpu) {
    let w = cpu.gbr();
    let v = cpu.read16(w + TVMD) as i16 as i32 as u32 | DISP;
    cpu.write16(w + TVMD, v as u16);
    cpu.leave(&[(0, v), (1, DISP)]);
}

pub fn sl_tv_off(cpu: &mut Cpu) {
    let w = cpu.gbr();
    let v = cpu.read16(w + TVMD) as i16 as i32 as u32 & 0x7fff;
    cpu.write16(w + TVMD, v as u16);
    cpu.write16(VDP2_TVMD, v as u16);
    cpu.leave(&[(0, v), (1, VDP2_TVMD)]);
}

/// SGL's copy of `bytes` from `src` to `dst` by the SCU's DMA (level 0: its add value 0x101 to
/// the A- or B-bus, 0x102 elsewhere; set going with interrupts held off once the last copy is
/// done: the copy flags' bit 2, or DSTA's level-0 bits clear, looked at every 48 counts), or by
/// slDMACopy (the CPU's DMA) when either is in low work RAM (the SCU cannot reach it). The copy
/// state byte set to (and the result) 3, or 1.
const COPY_FLAGS: u32 = 0xbb;
const COPY_STATE: u32 = 0xb8;
const COPY_DONE: u8 = 4;
const COPY_GOING: u8 = 16;
const SL_DMA_COPY: u32 = 0x0603_3a10;
const D0R: u32 = 0x25fe_0000;
const D0W: u32 = 0x25fe_0004;
const D0C: u32 = 0x25fe_0008;
const D0AD: u32 = 0x25fe_000c;
const D0EN: u32 = 0x25fe_0010;
const D0MD: u32 = 0x25fe_0014;
const DSTA: u32 = 0x25fe_007c;
const DSTA_LEVEL0: u32 = 0x1_0030;

pub fn scu_copy(cpu: &mut Cpu, dst: u32, src: u32, bytes: u32) -> u32 {
    let w = cpu.gbr();
    let low = |a: u32| (0x0020_0000..0x0030_0000).contains(&a) || (0x2020_0000..0x2030_0000).contains(&a);
    if low(src) || low(dst) {
        // the SH-2 code pushes PR for its call
        let pr = cpu.pr();
        cpu.push(&[pr]);
        cpu.call(SL_DMA_COPY, &[src, dst]);
        cpu.pop(4);
        cpu.write8(w + COPY_STATE, 1);
        return 1;
    }
    let near = (0x05a0_0000..0x05fe_0000).contains(&dst);
    let add = if near || (0x25a0_0000..0x25fe_0000).contains(&dst) { 0x101 } else { 0x102 };
    let t = loop {
        if cpu.read8(w + COPY_FLAGS) & COPY_DONE != 0 {
            break false;
        }
        if cpu.read32(DSTA) & DSTA_LEVEL0 == 0 {
            break true;
        }
        cpu.wait(0x0607_21b4, 200);
    };
    let sr = (cpu.sh2.sr & !1) | t as u32;
    cpu.push(&[sr]);
    cpu.sh2.sr |= 0xf0;
    cpu.write32(D0R, src);
    cpu.write32(D0W, dst);
    cpu.write32(D0C, bytes);
    cpu.write32(D0AD, add);
    cpu.write32(D0MD, 7);
    cpu.write8(w + COPY_FLAGS, COPY_GOING);
    cpu.write32(D0EN, 0x101);
    cpu.write8(w + COPY_STATE, 3);
    cpu.sh2.sr = sr;
    cpu.pop(4);
    cpu.leave(&[(1, 0x101), (2, if near { 0x05a0_0000 } else { 0x25a0_0000 }), (3, D0R), (7, add)]);
    3
}

/// slGouraudTblCopy (a VBlank callback): the Gouraud tables made since the last copy (least to
/// most) copied to VDP1's VRAM (8 bytes each, at the first's number on), the range emptied; and
/// SGL's other table (256 bytes) to the end of VDP1's VRAM when its flag asks.
const VDP1_VRAM: u32 = 0x25c0_0000;
const GOURAUD_EXTRA_FLAG: u32 = 0x0607_12d4;
const GOURAUD_EXTRA: u32 = 0x0607_1350;
const GOURAUD_EXTRA_AT: u32 = 0x25c7_ff00;
const GOURAUD_EXTRA_SIZE: u32 = 0x100;
const SCU_COPY: u32 = 0x0607_215c;

pub fn sl_gouraud_tbl_copy(cpu: &mut Cpu) {
    let (most, least) = (cpu.read32(GOURAUD + GOURAUD_MOST), cpu.read32(GOURAUD + GOURAUD_LEAST));
    let bytes = most.wrapping_sub(least).wrapping_add(1) << 3;
    if least != u32::MAX {
        let first = cpu.read32(GOURAUD + GOURAUD_FIRST);
        let dst = (first.wrapping_add(least) << 3).wrapping_add(VDP1_VRAM);
        let src = cpu.read32(GOURAUD + GOURAUD_TABLES).wrapping_add(least << 3);
        cpu.write32(GOURAUD + GOURAUD_MOST, 0);
        cpu.write32(GOURAUD + GOURAUD_LEAST, u32::MAX);
        cpu.call(SCU_COPY, &[dst, src, bytes]);
    } else {
        cpu.leave(&[(0, least), (1, GOURAUD), (6, bytes)]);
    }
    let flag = cpu.read32(GOURAUD_EXTRA_FLAG);
    if flag & 1 != 0 {
        cpu.call(SCU_COPY, &[GOURAUD_EXTRA_AT, GOURAUD_EXTRA, GOURAUD_EXTRA_SIZE]);
        cpu.write32(GOURAUD_EXTRA_FLAG, 0);
        cpu.leave(&[(0, GOURAUD_EXTRA_FLAG), (1, 0)]);
    } else {
        cpu.leave(&[(1, flag >> 1)]);
        cpu.leave_t(false);
    }
}

/// slScrAutoDisp: the scroll screens in `screens` (bits: NBG0-NBG3) shown, and VRAM's access
/// cycles worked out for them (CYCA0-CYCB1: 4 banks of 8 time slots, a code each, 0xe none).
/// Each screen's reads are listed (4 bytes each: kind, code, banks, slot), the banks they read
/// as bits: its pattern name reads (a count by its zoom, kind -2, code the screen), then for
/// each its cell reads (a count by its colours; kind the offset of the pattern name read they
/// follow, code 4 on); a bitmap's reads instead (kind -3, banks as many as it spans); the
/// vertical cell scroll's first (kind -4, code 12; NBG1's, code 13, takes a byte of SGL's table
/// for its kind -5, and so is a cell read of sorts). Each read in turn takes the first slot free
/// in all its banks (a cell read some slots after its pattern name's, NBG0's cell scroll the
/// first two, NBG1's the first three; four slots in all in hi-res), backing up to the read before
/// (to a later slot, its old one freed but its code left) when none is. Then in a bank split in
/// two, a slot unused in one half against a used one in the other closed (0xf), and the run of
/// slots both halves leave unused at the end closed at its start. 0, or -1 when they do not fit
/// (nothing set). SGL's helpers (bsr, on its registers) are taken inline.
const AUTO_LIST: u32 = 36;
const NO_ACCESS: u32 = 0x0e0e_0e0e;
const ZOOM_READS: u32 = 0x0606_e140;
const COLOUR_READS: u32 = 0x0606_e144;
const BANK_BITS: u32 = 0x0606_e130;
const AFTER_NAME: u32 = 0x0606_e14c;
const LATER_SLOTS: u32 = 0x0606_e15c;
const BITMAP_BANKS: u32 = 0x0606_e164;
const CYCA0: u32 = 0x060f_fcd0;
const ZMCTL: u32 = 0x158;
const SCRCTL: u32 = 0x15a;
const VCSTA: u32 = 0x15c;
const BGON_LOW: u32 = 0xe1;

pub fn sl_scr_auto_disp(cpu: &mut Cpu, screens: u32) -> u32 {
    let w = cpu.gbr();
    let sb = |cpu: &mut Cpu, at: u32| cpu.read8(at) as i8 as i32 as u32;
    let sw = |cpu: &mut Cpu, at: u32| cpu.read16(at) as i16 as i32 as u32;
    let slots = cpu.stack();
    let free = slots + 32;
    let list = slots + AUTO_LIST;
    cpu.write32(free, u32::MAX);
    for k in (0..8).rev() {
        cpu.write32(slots + 4 * k, NO_ACCESS);
    }
    // the reads listed
    let mut at = list;
    let push = |cpu: &mut Cpu, at: &mut u32, kind: u32, code: u32, banks: u32| {
        cpu.write8(*at, kind as u8);
        cpu.write8(*at + 1, code as u8);
        cpu.write8(*at + 2, banks as u8);
        *at += 4;
    };
    let mut r7 = cpu.reg(7);
    for n in 0..4 {
        if screens & 1 << n == 0 {
            continue;
        }
        let code = 4 + n;
        let (names, chctl, colours) = if n < 2 {
            let zoom = (sw(cpu, w + ZMCTL) >> (8 * n)) & 3;
            let names = sb(cpu, ZOOM_READS + zoom);
            let chctl = sw(cpu, w + CHCTLA) >> (8 * n);
            (names, chctl, (chctl >> 4) & 7)
        } else {
            let chctl = sw(cpu, w + CHCTLB);
            (1, chctl, (chctl >> (4 * n - 7)) & 1)
        };
        let cells = sb(cpu, COLOUR_READS + colours);
        let bank = |cpu: &mut Cpu, at: u32| {
            let a = cpu.read32(at);
            sb(cpu, BANK_BITS + ((a >> 17) & 3))
        };
        let map_bank = bank(cpu, w + SCREEN_MAPS + 4 * n);
        let cell_bank = bank(cpu, SCREEN_CELLS + 4 + 4 * n);
        r7 = map_bank;
        if n < 2 && (sw(cpu, w + SCRCTL) >> (8 * n)) & 1 != 0 {
            let table = sw(cpu, w + VCSTA);
            let banks = sb(cpu, BANK_BITS + (table & 3));
            let kind = if n == 0 { -4i32 as u32 } else { BANK_BITS };
            push(cpu, &mut at, kind, 12 + n, banks);
        }
        if n < 2 && sw(cpu, w + CHCTLA) >> (8 * n) & 2 != 0 {
            let size = (chctl >> 2) & if n == 0 { 31 } else { 15 };
            let spans = sb(cpu, BITMAP_BANKS + size);
            let mut b = map_bank;
            for _ in 1..spans {
                b <<= 1;
                r7 |= b;
            }
            for _ in 0..names * cells {
                push(cpu, &mut at, -3i32 as u32, code, r7);
            }
        } else {
            let first = at;
            for _ in 0..names {
                push(cpu, &mut at, -2i32 as u32, n, map_bank);
            }
            for k in 0..names {
                for _ in 0..cells {
                    push(cpu, &mut at, first + 4 * k - list, code, cell_bank);
                }
            }
        }
    }
    cpu.write8(at, 0xff);
    // each read given a slot: 0x0606ded8 the next read, 0x0606deea a try at it with the slots
    // allowed (r6), 0x0606dfa8 backing up
    enum Step {
        Next,
        Try,
        Back,
    }
    let (mut r1, mut r2, mut r3, mut r4, mut r5, mut r6) = (0, 0, 0, screens, 0, 0);
    let mut cur = list;
    let mut step = Step::Next;
    loop {
        match step {
            Step::Next => {
                r5 = sb(cpu, cur + 2);
                r6 = if sb(cpu, w + SCREEN_MODE) & 8 != 0 { 0xf } else { 0xff };
                step = Step::Try;
            }
            Step::Try => {
                let kind = sb(cpu, cur);
                if kind == u32::MAX {
                    break;
                }
                r4 = kind;
                if kind & 0x80 != 0 {
                    if kind == -4i32 as u32 {
                        r1 = 3;
                        r6 &= r1;
                    } else if kind == -5i32 as u32 {
                        r1 = if sw(cpu, w + SCRCTL) & 1 != 0 { 6 } else { 7 };
                        r6 &= r1;
                    }
                } else {
                    r1 = sb(cpu, list + kind + 3);
                    r2 = sb(cpu, cur + 1);
                    let mode = sb(cpu, w + SCREEN_MODE);
                    r3 = if r1 == 0 { 5 } else { 4 };
                    if r1 == 0 && (r2 == 4 || r2 == 5) {
                        r2 = if mode & 8 != 0 { 0xf } else { 0xff };
                    } else {
                        r1 = r1.wrapping_add(mode & 8);
                        r2 = sb(cpu, AFTER_NAME.wrapping_add(r1));
                    }
                    r6 &= r2;
                }
                for b in 0..4 {
                    if r5 & 1 << b != 0 {
                        r2 = sb(cpu, free + b);
                        r6 &= r2;
                    }
                }
                r1 = 16;
                if r6 == 0 {
                    step = Step::Back;
                    continue;
                }
                r7 = r6.trailing_zeros();
                r6 >>= r7 + 1;
                cpu.write8(cur + 3, r7 as u8);
                r3 = sb(cpu, cur + 1);
                r2 = !sb(cpu, BANK_BITS + r7);
                r4 = slots + r7;
                for b in 0..4 {
                    if r5 & 1 << b != 0 {
                        r6 = sb(cpu, free + b) & r2;
                        cpu.write8(free + b, r6 as u8);
                        r6 = r4 + 8 * b;
                        cpu.write8(r6, r3 as u8);
                    }
                }
                cur += 4;
                step = Step::Next;
            }
            Step::Back => {
                if cur == list {
                    cpu.leave(&[(1, r1), (2, r2), (3, r3), (4, r4), (5, r5), (6, r6), (7, r7)]);
                    cpu.leave_t(true);
                    return u32::MAX;
                }
                cur -= 4;
                let slot = sb(cpu, cur + 3);
                r2 = sb(cpu, BANK_BITS.wrapping_add(slot));
                r6 = sb(cpu, LATER_SLOTS.wrapping_add(slot));
                if sb(cpu, w + SCREEN_MODE) & 8 != 0 {
                    r6 &= 0xf;
                }
                r5 = sb(cpu, cur + 2);
                for b in 0..4 {
                    if r5 & 1 << b != 0 {
                        r3 = sb(cpu, free + b) | r2;
                        cpu.write8(free + b, r3 as u8);
                    }
                }
                r1 = 16;
                step = if r6 == 0 { Step::Back } else { Step::Try };
            }
        }
    }
    // the slots as the cycle registers' nibbles
    let mut s = slots;
    for k in 0..8 {
        r2 = 0;
        for _ in 0..4 {
            r1 = sb(cpu, s);
            r2 = r2 << 4 | r1;
            s += 1;
        }
        cpu.write16(CYCA0 + 2 * k, r2 as u16);
    }
    r7 = 0;
    let split = sb(cpu, w + RAMCTL);
    let mut cyc = CYCA0;
    for half in [1, 2] {
        if split & half != 0 {
            (r4, r5) = (cpu.read32(cyc), cpu.read32(cyc + 4));
            (r7, r3) = (0xf, 0xe);
            for _ in 0..8 {
                r2 = r5 & r7;
                if r4 & r7 == r3 {
                    if r2 != r3 {
                        r4 |= r7;
                    }
                } else if r2 == r3 {
                    r5 |= r7;
                }
                (r7, r3) = (r7 << 4, r3 << 4);
            }
            let mut top = 0xf000_0000u32;
            (r7, r3, r1) = (u32::MAX, 0xeeee_eeee, 8);
            loop {
                r2 = r4 & r7;
                r3 &= r7;
                r7 >>= 2;
                if r2 == r3 {
                    r4 |= top;
                    r5 |= top;
                    break;
                }
                top >>= 4;
                r1 -= 1;
                r7 >>= 2;
                if r1 == 0 {
                    break;
                }
            }
            cpu.write32(cyc, r4);
            cpu.write32(cyc + 4, r5);
        }
        cyc += 8;
    }
    cpu.write8(w + BGON_LOW, screens as u8);
    cpu.leave(&[(1, r1), (2, r2), (3, r3), (4, r4), (5, r5), (6, cyc), (7, r7)]);
    cpu.leave_t(false);
    0
}

/// The screen mode's bit 7 (SGL's slDynamicFrame, as far as can be told): cleared (and 1) when
/// `on` is 0; else, unless the screen mode's bit 4 is set (then cleared, and 0), set once VDP1
/// has ended a frame (EDSR's CEF): if it has not, a frame change forced (FBCR 3, SGL's change
/// word 0x303 meanwhile, its low byte 3 again after each 127-count wait) until it has.
const FRAME_CHANGE: u32 = 0x12;
const VDP1_FBCR: u32 = 0x25d0_0002;
const VDP1_EDSR: u32 = 0x25d0_0010;
const CEF: u32 = 2;

pub fn frame_mode(cpu: &mut Cpu, on: u32) -> u32 {
    let w = cpu.gbr();
    let mode = cpu.read8(w + SCREEN_MODE) as i8 as i32 as u32 & 0x7f;
    if on == 0 {
        cpu.write8(w + SCREEN_MODE, mode as u8);
        cpu.leave_t(true);
        return 1;
    }
    cpu.leave(&[(7, mode)]);
    cpu.leave_t(false);
    if mode & 16 != 0 {
        cpu.write8(w + SCREEN_MODE, mode as u8);
        return 0;
    }
    let ended = cpu.read16(VDP1_EDSR) as u32 & CEF != 0;
    let change = cpu.read16(w + FRAME_CHANGE);
    if !ended {
        // the SH-2 code keeps the change word on the stack meanwhile
        cpu.push(&[change as i16 as i32 as u32]);
        cpu.write16(w + FRAME_CHANGE, 0x303);
        cpu.write16(VDP1_FBCR, 3);
        while cpu.read16(VDP1_EDSR) as u32 & CEF == 0 {
            cpu.wait(0x0606_dd1a, 520);
            cpu.write8(w + FRAME_CHANGE + 1, 3);
        }
        cpu.pop(4);
        cpu.write16(w + FRAME_CHANGE, change);
        cpu.leave(&[(1, 3)]);
    }
    cpu.write8(w + SCREEN_MODE, (mode | 0x80) as u8);
    cpu.leave(&[(2, VDP1_EDSR - 16)]);
    1
}

/// SGL's polygon put done on the master (slPutPolygonS, as far as can be told; the ribbons'):
/// the polygon data at `pd` (its points, polygons with their normals, attributes: one shared
/// unless `each`) counted against the limits as slPutPolygon's, its points turned by the current
/// matrix into the points room (16 bytes each: the place's whole parts packed, then x, y and z;
/// after a first slot whose z SGL's code fills from the registers it was handed, r3 as the caller
/// left it), then each polygon facing the eye (or two-sided) given a depth by its attribute's
/// sort (bits 0-1: the last polygon's, the nearest point's, the farthest's, the mean; less the
/// window's near) and, unless past the far limit, its command put in the polygon buffer (36
/// bytes each, downward: the attribute's control word, colours, the texture's (bit 2 of the sort)
/// size word, the points' packed places, the Gouraud table, the depth); the commands sent to the
/// slave as one (command 40) when there are any. Whether the limits let it. SGL's code dispatches
/// on the sort through a table (0x06071684), and keeps the texture table in PR meanwhile.
const TEXTURES: u32 = 0x3c;
const NORMAL_SIZE_AT: u32 = 12;
const POINT_ROOM_SIZE: u32 = 16;
const POLYGON_ENTRY: u32 = 36;

pub fn put_polygons(cpu: &mut Cpu, pd: u32, each: u32) -> u32 {
    let w = cpu.gbr();
    let sp = cpu.stack();
    cpu.write8(w + PUT_FLAG, 1);
    let n = cpu.read16(w + POLYGONS_PUT).wrapping_add(1);
    cpu.write16(w + POLYGONS_PUT, n);
    let points = cpu.read32(pd + PD_POINTS);
    let count = cpu.read32(pd + PD_POINT_COUNT).wrapping_add(1);
    let limits = cpu.read32(w + LIMITS);
    let counts = cpu.read32(w + COUNTS);
    let points_after = (counts & 0xffff).wrapping_add(count);
    let swapped = points_after.rotate_left(16);
    if points_after > limits & 0xffff {
        cpu.leave(&[(1, points_after), (2, swapped), (4, pd + 8), (6, limits), (7, points)]);
        cpu.leave_t(true);
        return 0;
    }
    let polygons = cpu.read32(pd + PD_POLYGON_COUNT);
    let polygons_after = (counts >> 16).wrapping_add(polygons);
    let new_counts = (polygons_after << 16) | (swapped >> 16);
    if polygons_after > limits >> 16 {
        cpu.leave(&[(1, points_after), (2, new_counts), (4, pd + 8), (6, limits >> 16), (7, points)]);
        cpu.leave_t(true);
        return 0;
    }
    cpu.write32(sp - 4, new_counts);
    let room = cpu.read32(w + POINTS_ROOM);
    let c = cpu.read32(w + COMMANDS);
    let buffer = cpu.read32(w + POLYGON_BUFFER);
    cpu.write32(c + 12, count);
    cpu.write32(c + 4, buffer);
    let m = matrix(cpu);
    // the points turned: SGL's loop stores each z on the next pass (the first pass a slot's)
    let (mut point, mut at) = (points, room);
    let mut z = (new_counts << 16 | cpu.reg(3) >> 16).wrapping_add(m);
    let mut sum = 0;
    for _ in 0..count - 1 {
        cpu.write32(at + 12, z);
        at += POINT_ROOM_SIZE;
        let p = [0, 1, 2].map(|k| cpu.read32(point + 4 * k));
        let mut place = [0; 3];
        for (r, v) in place.iter_mut().enumerate() {
            let row = m + 16 * r as u32;
            sum = 0;
            for (k, &a) in (0..).zip(&p) {
                let e = cpu.read32(row + 4 * k);
                sum = mac(cpu, sum, a, e);
            }
            *v = middle(sum).wrapping_add(cpu.read32(row + 12));
        }
        cpu.write32(at + 4, place[0]);
        cpu.write32(at + 8, place[1]);
        cpu.write32(at, (place[0] & 0xffff_0000) | place[1] >> 16);
        z = place[2];
        point += 12;
    }
    cpu.write32(at + 12, z);
    cpu.write32(sp - 8, at);
    // the window's near (the depth's whole part over 2 to the level)
    let level = cpu.read8(w + WIN_FLAG) as i8 as i32;
    let depth = cpu.read32(w + WIN_DEPTH) >> 16;
    let near = if (1..32).contains(&level) { depth >> level } else { 0 };
    cpu.write32(sp - 12, near);
    let pr = cpu.pr();
    cpu.write32(sp - 20, pr);
    let textures = cpu.read32(w + TEXTURES).wrapping_add(4);
    let step = if each != 0 { 12 } else { 0 };
    let far = cpu.read16(w + FAR) as i16 as i32 as u32;
    let points = room + POINT_ROOM_SIZE;
    let (mut polygon, mut attr) = (cpu.read32(pd + PD_POLYGONS), cpu.read32(pd + PD_ATTRS));
    let (mut entry, mut put, mut sort_z, mut r5) = (buffer, 0u32, u32::MAX, each);
    let row2 = m + 32;
    for _ in 0..polygons {
        sum = 0;
        for k in 0..3 {
            let (a, e) = (cpu.read32(polygon + 4 * k), cpu.read32(row2 + 4 * k));
            sum = mac(cpu, sum, a, e);
        }
        let word = cpu.read16(attr) as u32;
        let sort = word & 7;
        if sum >= 0 && (word >> 8) & 1 == 0 {
            polygon += NORMAL_SIZE_AT + 8;
            attr += step;
            continue;
        }
        let corner = |cpu: &mut Cpu, k: u32| {
            let i = cpu.read16(polygon + NORMAL_SIZE_AT + 2 * k) as i16 as i32 as u32;
            points.wrapping_add(i << 4)
        };
        let depth_of = |cpu: &mut Cpu, p: u32| cpu.read16(p + 12) as i16 as i32;
        let c0 = corner(cpu, 0);
        let c1 = corner(cpu, 1);
        let shown = if sort & 3 == 0 {
            sort_z <= far
        } else {
            let zs = [c0, c1].map(|p| depth_of(cpu, p));
            let c2 = corner(cpu, 2);
            let c3 = corner(cpu, 3);
            let zs = [zs[0], zs[1], depth_of(cpu, c2), depth_of(cpu, c3)];
            let z = match sort & 3 {
                1 => *zs.iter().min().unwrap(),
                2 => *zs.iter().max().unwrap(),
                _ => zs.iter().sum::<i32>() >> 2,
            };
            r5 = near;
            sort_z = (z as u32).wrapping_sub(near);
            sort_z <= far
        };
        if shown {
            let c2 = corner(cpu, 2);
            let c3 = corner(cpu, 3);
            if sort & 4 != 0 {
                let texno = cpu.read16(attr + 2) as i16 as i32 as u32;
                let size = cpu.read32(textures.wrapping_add(texno << 3));
                cpu.write32(entry + 8, size);
            }
            let gouraud = cpu.read16(attr + 8);
            r5 = cpu.read32(attr + 4);
            cpu.write16(entry + 28, gouraud);
            cpu.write32(entry + 4, r5);
            for (k, c) in (0..).zip([c0, c1, c2, c3]) {
                let v = cpu.read32(c);
                cpu.write32(entry + 12 + 4 * k, v);
            }
            let control = cpu.read16(attr + 10);
            cpu.write16(entry, control);
            put += 1;
            cpu.write16(entry + 30, sort_z as u16);
            entry -= POLYGON_ENTRY;
        }
        polygon += NORMAL_SIZE_AT + 8;
        attr += step;
    }
    cpu.write32(sp - 24, put);
    let last = at;
    let c = cpu.read32(w + COMMANDS);
    let mut r1 = QUAD_COMMAND;
    let mut r3 = last;
    if put != 0 {
        cpu.write32(c + 8, put);
        cpu.write32(c, QUAD_COMMAND);
        let next = c + 16;
        cpu.write32(w + COMMANDS, next);
        cpu.write16(SLAVE_SIGNAL, next as u16);
        cpu.write32(w + POLYGON_BUFFER, entry);
        r3 = last + 16;
        cpu.write32(w + POINTS_ROOM, r3);
        cpu.write32(w + COUNTS, new_counts);
        r1 = SLAVE_SIGNAL;
    }
    cpu.leave(&[(1, r1), (2, put), (3, r3), (4, new_counts), (5, r5), (6, row2), (7, polygon)]);
    cpu.leave_mac(sum);
    cpu.leave_t(put == 0);
    1
}

/// slRandom: SGL's random number: its seed stepped (times 0x41c64e6d, plus 0x3039), the seed's top
/// half rounded by the bit below it. (SGL keeps MACL as it was.)
const SEED: u32 = 0x33c;

pub fn sl_random(cpu: &mut Cpu) -> u32 {
    let w = cpu.gbr();
    let s = cpu.read32(w + SEED).wrapping_mul(0x41c6_4e6d).wrapping_add(0x3039);
    cpu.write32(w + SEED, s);
    let macl = cpu.sh2.macl;
    cpu.leave(&[(1, 0), (3, macl)]);
    cpu.leave_t(false);
    (s >> 16) + (s >> 15 & 1)
}

/// The matrix at `m` made the unit matrix.
fn unit(cpu: &mut Cpu, m: u32) {
    for r in 0..3 {
        for k in 0..4 {
            cpu.write32(m + 16 * r + 4 * k, if r == k { 0x1_0000 } else { 0 });
        }
    }
}

/// slUnitMatrix: the matrix at `m` (the current one for 0) made the unit matrix. `m`, or 1.
pub fn sl_unit_matrix(cpu: &mut Cpu, m: u32) -> u32 {
    let at = if m == 0 { matrix(cpu) } else { m };
    unit(cpu, at);
    cpu.leave(&[(1, 0x1_0000), (2, 0)]);
    cpu.leave_t(m == 0);
    if m == 0 { 1 } else { m }
}

/// slInitMatrix: the matrix stack back to its bottom (SGL's, in its code), the unit matrix.
const MATRIX_STACK: u32 = 0x0603_517c;

pub fn sl_init_matrix(cpu: &mut Cpu) -> u32 {
    let w = cpu.gbr();
    cpu.write8(w + MATRIX_DEPTH, 0);
    cpu.write32(w + MATRIX, MATRIX_STACK);
    unit(cpu, MATRIX_STACK);
    cpu.leave(&[(1, 0x1_0000), (2, 0)]);
    1
}

/// slIntFunction: `f` the function SGL calls each VBlank (an empty one of its own for 0).
const INT_FUNCTION: u32 = 0x14;
const NO_FUNCTION: u32 = 0x0603_3a0c;

pub fn sl_int_function(cpu: &mut Cpu, f: u32) {
    let v = if f == 0 { NO_FUNCTION } else { f };
    let w = cpu.gbr();
    cpu.write32(w + INT_FUNCTION, v);
    cpu.leave(&[(0, v)]);
    cpu.leave_t(f == 0);
}

/// slTransferEntry: a copy of `size` bytes from `src` to `dst` added to SGL's VBlank transfers
/// (an SCU indirect DMA table, at most 20: size, destination, source with bit 31 on the last;
/// the list's ready byte clear while the last entry changes), interrupts held off meanwhile (the
/// SH-2 code pushes SR). Whether it was (not for 0 bytes or a full list).
const TRANSFER_COUNT: u32 = 0x18;
const TRANSFER_READY: u32 = 0x1a;
const TRANSFER_LIST: u32 = 0x0600_89d8;
const TRANSFERS_MOST: u32 = 20;
const LAST_TRANSFER: u32 = 0x8000_0000;

pub fn sl_transfer_entry(cpu: &mut Cpu, src: u32, dst: u32, size: u32) -> u32 {
    let w = cpu.gbr();
    let sp = cpu.stack();
    let sr = (cpu.sh2.sr & !1) | (size == 0) as u32;
    cpu.write32(sp - 4, sr);
    if size == 0 {
        cpu.leave_t(true);
        return 0;
    }
    let n = (cpu.read16(w + TRANSFER_COUNT) as i16 as i32 as u32).wrapping_add(1);
    cpu.leave_t(false);
    if n > TRANSFERS_MOST {
        cpu.leave(&[(1, TRANSFERS_MOST), (7, n)]);
        return 0;
    }
    let mut at = cpu.read32(TRANSFER_LIST);
    let mut r1 = TRANSFERS_MOST;
    if n != 1 {
        r1 = (n - 2) * 12;
        cpu.write8(w + TRANSFER_READY, 0);
        at += r1;
        let v = cpu.read32(at + 8) & !LAST_TRANSFER;
        cpu.write32(at + 8, v);
        at += 12;
    }
    cpu.write32(at, size);
    cpu.write32(at + 4, dst);
    cpu.write32(at + 8, src | LAST_TRANSFER);
    cpu.write16(w + TRANSFER_COUNT, n as u16);
    cpu.write8(w + TRANSFER_READY, 1);
    cpu.leave(&[(1, r1), (3, at), (4, src | LAST_TRANSFER), (7, n)]);
    1
}

/// slDMACopy, slDMAXCopy: `size` bytes from `src` to `dst` on the CPU's DMA channel 0, once its
/// last copy is done (its busy flag set meanwhile), not waited for; the cache purged after unless
/// `dst` is a cache-through address. slDMACopy takes the widest unit all three allow (16 bytes,
/// 4, 2, 1) and does nothing for 0 bytes; slDMAXCopy takes its channel control from SGL's table
/// by `mode` (0-21: units of 1 below 7, 2 below 14, else 4, the addresses aligned to them), and
/// whether the mode was one.
const DMA_FLAGS: u32 = 0xb9;
const DMA_BUSY: u8 = 16;
const CCR: u32 = 0xffff_fe92;
const CACHE_PURGE: u8 = 0x11;
const CACHE_THROUGH: u32 = 0x2000_0000;
const DMAX_MODES: u32 = 0x0603_3b14;

fn dma_copy(cpu: &mut Cpu, src: u32, dst: u32, units: u32, control: u32) -> bool {
    let w = cpu.gbr();
    cpu.read32(DMAOR);
    cpu.write32(CHCR0, 0);
    cpu.write8(DRCR0, 0);
    cpu.write32(SAR0, src);
    cpu.write32(DAR0, dst);
    cpu.write32(TCR0, units);
    cpu.write8(w + DMA_FLAGS, DMA_BUSY);
    cpu.write32(CHCR0, control);
    cpu.write32(DMAOR, DMA_ON);
    let through = dst >= CACHE_THROUGH;
    if !through {
        cpu.write8(CCR, CACHE_PURGE);
    }
    cpu.leave(&[(1, if through { 0xffff_ffa4 } else { 0xffff_fe90 }), (3, SAR0)]);
    cpu.leave_t(through);
    through
}

pub fn sl_dma_copy(cpu: &mut Cpu, src: u32, dst: u32, size: u32) {
    if size == 0 {
        cpu.leave(&[(1, SAR0)]);
        cpu.leave_t(true);
        return;
    }
    dma0_wait(cpu, 0x0603_3a16);
    let all = src | dst | size;
    let (control, units) = if all & 15 == 0 {
        (0x5e01, size >> 2)
    } else if all & 3 == 0 {
        (0x5a01, size >> 2)
    } else if all & 1 == 0 {
        (0x5601, size >> 1)
    } else {
        (0x5201, size)
    };
    let through = dma_copy(cpu, src, dst, units, control);
    cpu.leave(&[(0, if through { DMA_ON } else { CACHE_PURGE as u32 }), (2, control), (6, units)]);
}

pub fn sl_dmax_copy(cpu: &mut Cpu, src: u32, dst: u32, size: u32, mode: u32) -> u32 {
    if mode >= 22 {
        cpu.leave(&[(1, u32::MAX)]);
        cpu.leave_t(true);
        return 0;
    }
    let (mask, units) = match mode as i32 {
        ..7 => (u32::MAX, size),
        7..14 => (!1, size >> 1),
        _ => (!3, size >> 2),
    };
    let control = cpu.read16(DMAX_MODES + 2 * mode) as i16 as i32 as u32;
    dma0_wait(cpu, 0x0603_3ac8);
    dma_copy(cpu, src & mask, dst & mask, units, control);
    cpu.leave(&[(4, src & mask), (5, dst & mask), (6, units), (7, control)]);
    1
}

/// slSynch: the frame's end. The slave told to run SGL's frame function (command 48, then 4; a
/// stop, 44, left after them for the next command to take), SGL's random seed stirred, its PCM
/// streams and sound commands seen to (0x06034df2, 0x06034e4a); then SGL's wait function called
/// (its empty one: the machine runs on) until the VBlank makes the sync count negative; then the
/// next frame: the polygon buffer's half flipped (when 0x06004068 says), the points room and the
/// commands back to SGL's areas, the counts cleared, window 0's limits made current (as
/// slCurWindow's, its far limit to FAR), the PCM streams again, and the sync count advanced by the
/// frames a sync takes. That count.
const SYNCH_A: u32 = 0xa8;
const SYNCH_B: u32 = 0xa6;
const FRAME_COMMAND: u32 = 48;
const FRAME_FUNCTION: u32 = 0x0603_3a6a;
const FRAME_COMMAND_TAIL: [u32; 2] = [4, 44];
const PCM_STEP: u32 = 0x0603_4df2;
const SOUND_FLUSH: u32 = 0x0603_4e4a;
const SL_RANDOM: u32 = 0x0603_4dd4;
const WAIT_FUNCTION: u32 = 0x3c8;
const EMPTY_WAIT: u32 = 0x0603_3654;
const SYNCH_COUNT: u32 = 0x13;
const SYNCH_FRAMES: u32 = 0x12;
const BUFFER_SIDE: u32 = 0x40;
const BUFFER_FLIP: u32 = 0x0600_4068;
const POLYGON_AREA: u32 = 0x0600_89c4;
const POINT_AREA: u32 = 0x0600_89c8;
const COMMAND_AREA: u32 = 0x0600_89d0;
const SYNCH_WINDOW_CLEAR: u32 = 0xa2;

pub fn sl_synch(cpu: &mut Cpu) -> u32 {
    let w = cpu.gbr();
    cpu.write8(w + SYNCH_A, 0);
    cpu.write8(w + SYNCH_B, 0);
    let c = cpu.read32(w + COMMANDS);
    cpu.write32(c + 8, 0);
    cpu.write32(c, FRAME_COMMAND);
    cpu.write32(c + 4, FRAME_FUNCTION);
    cpu.write32(c + 12, FRAME_COMMAND_TAIL[0]);
    cpu.write32(c + 16, FRAME_COMMAND_TAIL[1]);
    let next = c + 16;
    cpu.write32(w + COMMANDS, next);
    cpu.write16(SLAVE_SIGNAL, next as u16);
    cpu.leave(&[(0, next), (1, SLAVE_SIGNAL), (2, FRAME_FUNCTION), (5, 0)]);
    cpu.call(SL_RANDOM, &[]);
    cpu.call(PCM_STEP, &[]);
    cpu.call(SOUND_FLUSH, &[]);
    loop {
        let f = cpu.read32(w + WAIT_FUNCTION);
        if f == EMPTY_WAIT {
            cpu.wait(0x0603_4cfe, 10);
        } else {
            cpu.call(f, &[]);
        }
        if (cpu.read8(w + SYNCH_COUNT) as i8) < 0 {
            break;
        }
    }
    let flip = cpu.read32(BUFFER_FLIP) >> 1;
    let side = cpu.read32(w + BUFFER_SIDE) ^ flip;
    cpu.write32(w + BUFFER_SIDE, side);
    let area = cpu.read32(POLYGON_AREA).wrapping_add(flip);
    cpu.write32(w + POLYGON_BUFFER, side.wrapping_add(area).wrapping_sub(POLYGON_ENTRY));
    let points = cpu.read32(POINT_AREA) + POINT_ROOM_SIZE;
    cpu.write32(w + POINTS_ROOM, points);
    let commands = cpu.read32(COMMAND_AREA) | CACHE_THROUGH;
    cpu.write32(w + COMMANDS, commands);
    cpu.write16(SLAVE_SIGNAL, commands as u16);
    cpu.write32(w + COUNTS, 0);
    cpu.write16(w + POLYGONS_PUT, 0);
    let points = cpu.read32(POINT_AREA) + POINT_ROOM_SIZE;
    cpu.write32(w + POINTS_ROOM, points);
    // window 0's limits
    let swap = |v: u32| v.rotate_left(16);
    let flag = cpu.read16(WINDOWS + W_FLAG);
    cpu.write8(w + WIN_FLAG, flag as u8);
    let left = cpu.read32(WINDOWS + W_LEFT);
    let limit = cpu.read16(WINDOWS + W_CENTRE);
    cpu.write16(w + FAR, limit);
    let top = cpu.read32(WINDOWS + W_TOP);
    let depth = cpu.read32(WINDOWS + W_DEPTH);
    cpu.write32(w + WIN_DEPTH, depth);
    let right = cpu.read32(WINDOWS + W_RIGHT);
    let screen = cpu.read32(w + WIN_SCREEN);
    let low = right.wrapping_sub(left) << 16;
    let high = swap(right).wrapping_sub(swap(left));
    cpu.write32(w + WIN_WIDTH, (high << 16) | (low >> 16));
    let high = high.wrapping_add(screen);
    cpu.write32(w + WIN_SCREEN_X, (high << 16) | (screen.wrapping_add(low) >> 16));
    let size = 0x1_0001u32.wrapping_add(top).wrapping_sub(left);
    cpu.write32(w + WIN_SIZE, size);
    let twice = swap(screen).wrapping_mul(2);
    cpu.write32(w + WIN_SIZE_SCREEN, size.wrapping_add(twice));
    let corner = |a: u32| (swap(a).wrapping_sub(swap(right)), a.wrapping_sub(right));
    let (rt, rt_low) = corner(top);
    cpu.write32(w + WIN_RIGHT_TOP, (rt << 16) | (rt_low & 0xffff));
    let (lt, lt_low) = corner(left);
    cpu.write32(w + WIN_LEFT_TOP, (lt << 16) | (lt_low & 0xffff));
    cpu.write16(w + CUR_WINDOW, 0);
    cpu.write16(w + SYNCH_WINDOW_CLEAR, 0);
    cpu.leave(&[(0, 0), (1, lt), (2, right), (3, high), (4, lt_low), (5, rt_low), (6, twice)]);
    cpu.call(PCM_STEP, &[]);
    let frames = cpu.read8(w + SYNCH_FRAMES) as i8 as i32 as u32;
    let count = (cpu.read8(w + SYNCH_COUNT) as i8 as i32 as u32).wrapping_add(frames);
    cpu.write8(w + SYNCH_COUNT, count as u8);
    cpu.leave(&[(1, frames)]);
    count
}

/// SGL's sound commands sent (as many as it has queued, 16 bytes each, counted at GBR+0xb5):
/// once the sound CPU has taken the last ones (its flag at 0x25a004e0 no longer negative, looked
/// at every 48 counts), copied on DMA channel 0 (once it is free) from SGL's buffer (made the
/// buffer again) to the sound CPU's, the flag set, the count cleared. (Not seen run past its
/// count: the game sends its own commands.)
const SOUND_COUNT: u32 = 0xb5;
const SOUND_BUFFER: u32 = 0x370;
const SOUND_MAKER: u32 = 0x374;
const SOUND_FLAG: u32 = 0x25a0_04e0;
const SOUND_COMMANDS: u32 = 0x25a0_0700;
const SGL_SOUND_BUFFER: u32 = 0x0603_4fe8;

pub fn sound_flush(cpu: &mut Cpu) {
    let w = cpu.gbr();
    let n = cpu.read8(w + SOUND_COUNT) as i8 as i32 as u32;
    if n as i32 <= 0 {
        cpu.leave(&[(0, n), (6, n << 2)]);
        cpu.leave_t(false);
        return;
    }
    let words = n << 3;
    while (cpu.read8(SOUND_FLAG) as i8) < 0 {
        cpu.wait(0x0603_4e60, 100);
    }
    cpu.write32(w + SOUND_BUFFER, SGL_SOUND_BUFFER);
    dma0_wait(cpu, 0x0603_4e72);
    dma0_wait(cpu, 0x0603_4e7c);
    cpu.read32(DMAOR);
    cpu.write32(CHCR0, 0);
    cpu.write8(DRCR0, 0);
    cpu.write32(SAR0, SGL_SOUND_BUFFER);
    cpu.write32(DAR0, SOUND_COMMANDS);
    cpu.write32(TCR0, words);
    cpu.write32(CHCR0, COPY_WORDS);
    cpu.write32(DMAOR, DMA_ON);
    cpu.write8(SOUND_FLAG, 0x80);
    cpu.write8(w + SOUND_COUNT, 0);
    cpu.leave(&[(0, 0), (1, SAR0), (2, SOUND_FLAG), (3, SAR0), (4, SGL_SOUND_BUFFER), (5, SOUND_COMMANDS), (6, words)]);
    cpu.leave_t(false);
}

/// SGL's PCM streams (8 slots of 20 bytes): each one marked done ('p' first) has its channel's bit
/// (from its number and its place in sound RAM: SGL's tables) cleared in SGL's channel mask, the
/// slot cleared, and a command (0x86, with the stream's channel bits) made in the sound command
/// buffer by SGL's command function ([GBR+0x374]); the commands sent (sound_flush; SGL's code jumps
/// there to return) if any were made. (Not seen run past the slots' marks: the game has no PCM
/// streams.)
const PCM_SLOTS: u32 = 0x0603_4f48;
const PCM_SLOT_SIZE: u32 = 20;
const PCM_DONE: u8 = 0x70;
const PCM_COMMAND: u32 = 0x86;
const PCM_CHANNELS: u32 = 0xb2;
const PCM_FIRST: u32 = 0xb1;
const PCM_BITS: u32 = 0x0603_4f2c;
const PCM_STARTS: u32 = 0x0603_4f34;
const PCM_BASE: u32 = 0x0600_89d4;

pub fn pcm_step(cpu: &mut Cpu) {
    let w = cpu.gbr();
    let sb = |cpu: &mut Cpu, at: u32| cpu.read8(at) as i8 as i32 as u32;
    let buffer = cpu.read32(w + SOUND_BUFFER);
    let maker = cpu.read32(w + SOUND_MAKER);
    let mut made = 0u32;
    cpu.push(&[made]);
    let mut slot = PCM_SLOTS;
    for n in (1..=8u32).rev() {
        if cpu.read8(slot) == PCM_DONE {
            let b = sb(cpu, slot + 1);
            cpu.push(&[b, buffer, n]);
            // 0x06034eae: the stream's channel bit cleared
            let k = (b.rotate_left(1)) & 7;
            let bit = sb(cpu, PCM_BITS + k);
            let base = cpu.read32(PCM_BASE);
            let start = cpu.read16(PCM_STARTS + 2 * k) as i16 as i32 as u32;
            let at = cpu.read32(slot + 8).wrapping_sub(base).wrapping_add(start);
            let place = ((at << 3) >> 16) & 15;
            let shift = (sb(cpu, w + PCM_FIRST).wrapping_sub(1)).wrapping_sub(place);
            let bit = if shift >= 32 { 0 } else { bit << shift };
            let mask = (cpu.read16(w + PCM_CHANNELS) as i16 as i32 as u32) & !bit;
            cpu.write16(w + PCM_CHANNELS, mask as u16);
            cpu.pop(8);
            cpu.write8(buffer + 2, ((b >> 4) & 7) as u8);
            cpu.write16(slot, 0);
            cpu.write8(buffer, PCM_COMMAND as u8);
            cpu.leave(&[(0, PCM_COMMAND as i8 as i32 as u32), (1, start), (2, buffer), (3, 15), (4, b)]);
            cpu.leave(&[(5, place), (6, slot), (7, maker)]);
            cpu.call(maker, &[]);
            cpu.pop(4);
            made += 1;
            let sp = cpu.stack();
            cpu.write32(sp, made);
        }
        slot += PCM_SLOT_SIZE;
    }
    cpu.pop(4);
    if made != 0 {
        cpu.call(SOUND_FLUSH, &[]);
    } else {
        cpu.leave(&[(0, 0), (1, 0), (2, buffer), (6, slot), (7, maker)]);
        cpu.leave_t(true);
    }
}

/// SGL's SCU DMA end interrupts (levels 0-2, from the BIOS's dispatch): the level's copy flags'
/// bits 1-2 set (bit 2 is what scu_copy waits on). The SH-2 code addresses the flags from GBR
/// meanwhile.
const DMA_LEVEL_FLAGS: u32 = 0x060f_fcbb;

fn dma_level_end(cpu: &mut Cpu, level: u32) {
    let at = DMA_LEVEL_FLAGS + level;
    let v = cpu.read8(at) | 6;
    cpu.write8(at, v);
}

pub fn dma_level0_end(cpu: &mut Cpu) {
    dma_level_end(cpu, 0);
}

pub fn dma_level1_end(cpu: &mut Cpu) {
    dma_level_end(cpu, 1);
}

pub fn dma_level2_end(cpu: &mut Cpu) {
    dma_level_end(cpu, 2);
}

/// SGL's SMPC interrupt (from the BIOS's dispatch): the SMPC's status (its SR's top half, low
/// bits 12) kept; with SR's bit 7 clear the status registers copied out (OREG0, then OREG1-15
/// from SGL's status's fourth byte), else the ST-V's inputs made Saturn pads (two pads from the
/// player ports, then the two extra ports) at SGL's peripheral data, the OREGs after them; then
/// INTBACK continued (bit 5: IREG0's continue bit flipped) or, at its end (bit 7), SGL's
/// peripheral flag taken and its byte's bit 6 set. The SH-2 code saves r0-r4, PR, MACH and MACL.
const SMPC_SR: u32 = 0x2010_0061;
const SMPC_OREG0: u32 = 0x2010_0021;
const SMPC_IREG0: u32 = 0x2010_0001;
const STV_PORTS: u32 = 0x2040_0001;
const SMPC_STATUS: u32 = 0x0603_4564;
const PERIPHERALS: u32 = 0x0603_4538;
const PERIPHERAL_FILL: u32 = 0x060f_ff98;
const PERIPHERAL_MOST: u32 = 0x200;
const SMPC_KEPT: u32 = 0x060f_ffbe;
const INTBACK: u32 = 0x060f_ffbf;
const PERIPHERAL_FLAG: u32 = 0x060f_ffb9;

pub fn smpc_interrupt(cpu: &mut Cpu) {
    let sp = cpu.stack();
    let status = (cpu.read8(SMPC_SR) as i8 as i32 as u32 & 0xf0) | 12;
    if status & 0x80 == 0 {
        smpc_status(cpu);
    } else {
        smpc_pads(cpu, sp, status);
    }
    cpu.write8(SMPC_KEPT, status as u8);
    if status & 0x20 != 0 {
        let v = cpu.read8(INTBACK) ^ 0x80;
        cpu.write8(INTBACK, v);
        cpu.write8(SMPC_IREG0, v);
    } else {
        let taken = cpu.read8(PERIPHERAL_FLAG);
        cpu.write8(PERIPHERAL_FLAG, taken | 0x80);
        if status & 0x80 != 0 {
            let v = cpu.read8(INTBACK) | 0x40;
            cpu.write8(INTBACK, v);
            cpu.write8(PERIPHERAL_FLAG, v);
        }
    }
}

/// 0x06034ab0: the status registers copied out.
fn smpc_status(cpu: &mut Cpu) {
    let byte = |cpu: &mut Cpu, k: u32| cpu.read8(SMPC_OREG0 + 2 * k);
    let v = byte(cpu, 0);
    cpu.write8(SMPC_STATUS, v);
    for k in 1..8 {
        let v = byte(cpu, k);
        cpu.write8(SMPC_STATUS + 3 + k, v);
    }
    for k in 8..16 {
        let v = byte(cpu, k);
        cpu.write8(SMPC_STATUS + 4 + k, v);
    }
}

/// 0x06034aec: the ST-V's inputs as Saturn pads (2 bytes each, active low: the ports' bits moved
/// to the pad's places), unless SGL's peripheral data is full; `frame` where the SH-2 code's
/// pushes go (below the handler's own: the status it holds in r4, its return into the handler,
/// the registers it uses).
const SMPC_PADS_RETURN: u32 = 0x0603_4c44;

fn smpc_pads(cpu: &mut Cpu, frame: u32, status: u32) {
    let sp = frame - 4;
    cpu.write32(sp, status);
    let fill = cpu.read32(PERIPHERAL_FILL);
    if fill >= PERIPHERAL_MOST {
        cpu.write32(PERIPHERAL_FILL, fill);
        return;
    }
    let base = cpu.read32(PERIPHERALS);
    for (k, v) in [SMPC_PADS_RETURN, base, SMPC_OREG0, 32, fill].into_iter().enumerate() {
        cpu.write32(sp - 4 - 4 * k as u32, v);
    }
    let [r13, r12, r11, r10, _, r8] = [13, 12, 11, 10, 9, 8].map(|n| cpu.reg(n));
    let r14 = cpu.caller_r14;
    for (k, v) in [r14, r13, r12, r11, r10, r8].into_iter().enumerate() {
        cpu.write32(sp - 24 - 4 * k as u32, v);
    }
    let port = |cpu: &mut Cpu, k: u32| cpu.read8(STV_PORTS + k) as i8 as i32 as u32;
    let (p1, p2, p3, p4, system) = (port(cpu, 0), port(cpu, 2), port(cpu, 4), port(cpu, 8), port(cpu, 10));
    let mut at = base;
    let mut put = |cpu: &mut Cpu, v: u32| {
        cpu.write8(at, v as u8);
        at += 1;
    };
    put(cpu, 4);
    put(cpu, 2);
    for v in pad_bytes(p1, system, p3 >> 1) {
        put(cpu, v);
    }
    put(cpu, 2);
    for v in pad_bytes(p2, system >> 4, p3 >> 2) {
        put(cpu, v);
    }
    put(cpu, 2);
    for v in extra_bytes(p4) {
        put(cpu, v);
    }
    put(cpu, 2);
    for v in extra_bytes(system) {
        put(cpu, v);
    }
    put(cpu, 0xf0);
    let start = fill + 14;
    for k in 0..18 {
        let v = cpu.read8(SMPC_OREG0 + 28 + 2 * k);
        cpu.write8(base.wrapping_add(start + k), v);
    }
    cpu.write32(PERIPHERAL_FILL, start + 18);
    let r6 = (p3 >> 2) | 0xffff_fff7;
    cpu.leave(&[(5, (system >> 4) >> 3), (6, r6)]);
}

/// 0x06034ba4: a player port (`p`), its start and coin bits from the system port (`s`, low bits)
/// and a service bit (`x`, bit 0) as a pad's two bytes.
fn pad_bytes(p: u32, s: u32, x: u32) -> [u32; 2] {
    let mut v = (p << 1 | 0xffff_ff5f) & (p >> 1 | 0xffff_ffac) & (p << 2 | 0xffff_fffb) & (x | 0xffff_fff7);
    let mut r2 = p >> 1 | 0xffff_ffac;
    let mut s = s;
    for _ in 0..3 {
        r2 = r2 << 1 | (s & 1);
        s >>= 1;
    }
    v &= r2 << 12 | 0xffff_8fff;
    [v, v >> 8]
}

/// 0x06034be8: an extra port as a pad's two bytes.
fn extra_bytes(p: u32) -> [u32; 2] {
    let v = (p << 1 | 0xffff_ff5f) & (p >> 1 | 0xffff_ffac) & (p << 2 | 0xffff_fffb) & (p | 0xffff_fff7);
    [v, v >> 8]
}

/// SGL's SMPC layer: its lock (a byte taken by TAS.B), its state byte (0x060fffba: bit 0 the SMPC
/// busy too long, bit 1 a command going, bit 2 INTBACK's peripheral data wanted), the settings it
/// keeps for the SMPC's ports (0x060fffc0-0x060fffc3).
const SMPC_LOCK: u32 = 0x060f_ffb8;
const SMPC_STATE: u32 = 0x060f_ffba;
const SMPC_BUSY_FRAMES: u32 = 0x060f_ffbb;
const SMPC_PORT_SETTINGS: u32 = 0x060f_ffc0;
const SMPC_EXLE: u32 = 0x2010_007f;
const SMPC_LOCK_TAKE: u32 = 0x0603_3b40;
const SMPC_LOCK_GIVE: u32 = 0x0603_3b4a;
const SMPC_GOING: u32 = 0x0603_3f02;

/// SGL's SMPC lock taken: 0, or -1 when it was held.
pub fn smpc_lock(cpu: &mut Cpu) -> u32 {
    let v = cpu.read8(SMPC_LOCK);
    cpu.write8(SMPC_LOCK, v | 0x80);
    cpu.leave(&[(3, SMPC_LOCK)]);
    cpu.leave_t(v == 0);
    if v == 0 { 0 } else { u32::MAX }
}

/// SGL's SMPC lock given back: `v` (what its callers return through it).
pub fn smpc_unlock(cpu: &mut Cpu, v: u32) -> u32 {
    cpu.write8(SMPC_LOCK, 0);
    cpu.leave(&[(1, 0), (3, SMPC_LOCK)]);
    v
}

/// Whether the SMPC state's bit is clear (0) or set (-1): bit 2 INTBACK's data wanted, bit 1 a
/// command going.
fn smpc_state_bit(cpu: &mut Cpu, bit: u8) -> u32 {
    let set = cpu.read8(SMPC_STATE) & bit != 0;
    cpu.leave(&[(2, SMPC_STATE)]);
    cpu.leave_t(!set);
    if set { u32::MAX } else { 0 }
}

pub fn smpc_wanted(cpu: &mut Cpu) -> u32 {
    smpc_state_bit(cpu, 4)
}

pub fn smpc_going(cpu: &mut Cpu) -> u32 {
    smpc_state_bit(cpu, 2)
}

/// A command going marked in the SMPC state.
pub fn smpc_mark_going(cpu: &mut Cpu) {
    let v = cpu.read8(SMPC_STATE) | 2;
    cpu.write8(SMPC_STATE, v);
    cpu.leave(&[(0, v as i8 as i32 as u32), (2, SMPC_STATE)]);
}

/// The SMPC's busy frames counted (SR's bit 4): at three, the state's bit 0 set and the count
/// begun again. SR's byte, or 3, or the state.
pub fn smpc_busy_count(cpu: &mut Cpu) -> u32 {
    let sr = cpu.read8(SMPC_SR) as i8 as i32 as u32;
    let mut n = cpu.read8(SMPC_BUSY_FRAMES) as i8 as i32 as u32;
    if sr & 16 == 0 {
        cpu.leave(&[(1, n), (2, SMPC_SR), (3, SMPC_BUSY_FRAMES)]);
        cpu.leave_t(true);
        return sr;
    }
    n = n.wrapping_add(1);
    let mut r0 = 3;
    cpu.leave_t(3 > n);
    if 3 <= n {
        let v = cpu.read8(SMPC_STATE) | 1;
        cpu.write8(SMPC_STATE, v);
        (r0, n) = (v as i8 as i32 as u32, 0);
    }
    cpu.write8(SMPC_BUSY_FRAMES, n as u8);
    cpu.leave(&[(1, n), (2, SMPC_STATE), (3, SMPC_BUSY_FRAMES)]);
    r0
}

/// A setting of SGL's for the SMPC's ports made under its lock: only while no command is going
/// (Going::Check), or that asked and not heeded (Going::Ask), or not asked; what the lock or the
/// check said, or 0.
enum Going {
    Check,
    Ask,
    No,
}

fn smpc_setting(cpu: &mut Cpu, going: Going, set: impl FnOnce(&mut Cpu)) -> u32 {
    if cpu.call(SMPC_LOCK_TAKE, &[]) != 0 {
        return u32::MAX;
    }
    if !matches!(going, Going::No) {
        let r = cpu.call(SMPC_GOING, &[]);
        if matches!(going, Going::Check) && r != 0 {
            return cpu.call(SMPC_LOCK_GIVE, &[r]);
        }
    }
    set(cpu);
    cpu.call(SMPC_LOCK_GIVE, &[0])
}

/// The port settings' first two bytes (7 bits).
pub fn smpc_port_setting_a(cpu: &mut Cpu, v: u32) -> u32 {
    smpc_setting(cpu, Going::Check, |cpu| cpu.write8(SMPC_PORT_SETTINGS, (v & 0x7f) as u8))
}

pub fn smpc_port_setting_b(cpu: &mut Cpu, v: u32) -> u32 {
    smpc_setting(cpu, Going::Check, |cpu| cpu.write8(SMPC_PORT_SETTINGS + 1, (v & 0x7f) as u8))
}

/// A bit of the third or fourth setting byte set when `v`'s byte is 0, cleared otherwise (the
/// fourth's also written to EXLE, with no check of a command going).
fn smpc_setting_bit(cpu: &mut Cpu, at: u32, bit: u8, v: u32, exle: bool) -> u32 {
    smpc_setting(cpu, if exle { Going::Ask } else { Going::Check }, |cpu| {
        let b = cpu.read8(at);
        let b = if v & 0xff == 0 { b | bit } else { b & !bit };
        cpu.write8(at, b);
        if exle {
            cpu.write8(SMPC_EXLE, b);
        }
        cpu.leave(&[(1, 0), (2, at), (4, v & 0xff)]);
    })
}

pub fn smpc_port_bit_c0(cpu: &mut Cpu, v: u32) -> u32 {
    smpc_setting_bit(cpu, SMPC_PORT_SETTINGS + 2, 1, v, false)
}

pub fn smpc_port_bit_c1(cpu: &mut Cpu, v: u32) -> u32 {
    smpc_setting_bit(cpu, SMPC_PORT_SETTINGS + 2, 2, v, false)
}

pub fn smpc_port_bit_d0(cpu: &mut Cpu, v: u32) -> u32 {
    smpc_setting_bit(cpu, SMPC_PORT_SETTINGS + 3, 1, v, true)
}

pub fn smpc_port_bit_d1(cpu: &mut Cpu, v: u32) -> u32 {
    smpc_setting_bit(cpu, SMPC_PORT_SETTINGS + 3, 2, v, true)
}

/// The SMPC command `c`'s entry in SGL's table (4 bytes each). SGL's code multiplies on MACL.
const SMPC_COMMANDS: u32 = 0x0603_3cf8;

pub fn smpc_command_entry(cpu: &mut Cpu, c: u32) -> u32 {
    let c = c & 0xff;
    cpu.sh2.macl = c * 4;
    cpu.leave(&[(1, 4), (2, SMPC_COMMANDS), (4, c)]);
    SMPC_COMMANDS + c * 4
}

/// SGL's peripheral entries (30, its stride apart) marked empty.
const PERIPHERAL_ENTRIES: u32 = 0x0603_453c;
const PERIPHERAL_STRIDE: u32 = 0x0603_4540;

pub fn peripherals_clear(cpu: &mut Cpu) {
    let mut at = cpu.read32(PERIPHERAL_ENTRIES);
    let stride = cpu.read16(PERIPHERAL_STRIDE) as i16 as i32 as u32;
    for _ in 0..30 {
        cpu.write8(at, 0xff);
        at = at.wrapping_add(stride);
    }
    cpu.leave(&[(0, u32::MAX), (1, 0), (2, stride), (3, at)]);
    cpu.leave_t(true);
}

/// The SMPC port settings' getters (SGL's): the first two bytes' 7 bits; whether a bit of the
/// third or fourth byte is clear (1).
fn smpc_setting_value(cpu: &mut Cpu, at: u32) -> u32 {
    cpu.leave(&[(2, at)]);
    cpu.read8(at) as i8 as i32 as u32 & 0x7f
}

fn smpc_setting_clear(cpu: &mut Cpu, at: u32, bit: u8) -> u32 {
    let clear = cpu.read8(at) & bit == 0;
    cpu.leave(&[(2, at)]);
    cpu.leave_t(clear);
    clear as u32
}

pub fn smpc_setting_a(cpu: &mut Cpu) -> u32 {
    smpc_setting_value(cpu, SMPC_PORT_SETTINGS)
}

pub fn smpc_setting_b(cpu: &mut Cpu) -> u32 {
    smpc_setting_value(cpu, SMPC_PORT_SETTINGS + 1)
}

pub fn smpc_setting_c0_clear(cpu: &mut Cpu) -> u32 {
    smpc_setting_clear(cpu, SMPC_PORT_SETTINGS + 2, 1)
}

pub fn smpc_setting_c1_clear(cpu: &mut Cpu) -> u32 {
    smpc_setting_clear(cpu, SMPC_PORT_SETTINGS + 2, 2)
}

pub fn smpc_setting_d0_clear(cpu: &mut Cpu) -> u32 {
    smpc_setting_clear(cpu, SMPC_PORT_SETTINGS + 3, 1)
}

pub fn smpc_setting_d1_clear(cpu: &mut Cpu) -> u32 {
    smpc_setting_clear(cpu, SMPC_PORT_SETTINGS + 3, 2)
}

/// Whether the ports' settings let peripheral data be read: for each port whose third-byte bit is
/// clear, its setting byte 0 and its fourth-byte bit clear (else -1). 0 or -1.
const SMPC_SETTING_A: u32 = 0x0603_3b58;
const SMPC_SETTING_B: u32 = 0x0603_3b60;
const SMPC_C0_CLEAR: u32 = 0x0603_3bf0;
const SMPC_C1_CLEAR: u32 = 0x0603_3bfa;
const SMPC_D0_CLEAR: u32 = 0x0603_3c6c;
const SMPC_D1_CLEAR: u32 = 0x0603_3c76;

pub fn smpc_ports_readable(cpu: &mut Cpu) -> u32 {
    for (clear, setting, other) in [(SMPC_C0_CLEAR, SMPC_SETTING_A, SMPC_D0_CLEAR), (SMPC_C1_CLEAR, SMPC_SETTING_B, SMPC_D1_CLEAR)] {
        if cpu.call(clear, &[]) == 1 && (cpu.call(setting, &[]) != 0 || cpu.call(other, &[]) != 1) {
            return u32::MAX;
        }
    }
    0
}

/// 0 when either port's third-byte bit is clear, else -1.
pub fn smpc_any_port(cpu: &mut Cpu) -> u32 {
    if cpu.call(SMPC_C0_CLEAR, &[]) == 1 || cpu.call(SMPC_C1_CLEAR, &[]) == 1 {
        return 0;
    }
    u32::MAX
}

/// `v` with each port's mode bits set where its third-byte bit is set (port 1 0x30, port 2 the
/// byte's top two and all above, as SGL's sign-extended constant has it).
pub fn smpc_port_modes(cpu: &mut Cpu, v: u32) -> u32 {
    let mut v = v;
    if cpu.call(SMPC_C0_CLEAR, &[]) != 1 {
        v |= 0x30;
    }
    if cpu.call(SMPC_C1_CLEAR, &[]) != 1 {
        v |= 0xffff_ffc0;
    }
    v
}

/// The SMPC's mode byte (0x060fffc4): bit 1 set when `v`'s byte is 0, cleared otherwise; or port
/// 1's two mode bits (4-5) or port 2's (6-7) set to `v` (above 3 taken as 0; 2 refused, -1).
const SMPC_MODES: u32 = 0x060f_ffc4;

pub fn smpc_mode_bit(cpu: &mut Cpu, v: u32) -> u32 {
    smpc_setting(cpu, Going::No, |cpu| {
        let b = cpu.read8(SMPC_MODES);
        let b = if v & 0xff == 0 { b | 2 } else { b & !2 };
        cpu.write8(SMPC_MODES, b);
        cpu.leave(&[(1, 0), (2, SMPC_MODES)]);
    })
}

fn smpc_port_mode(cpu: &mut Cpu, v: u32, shift: u32) -> u32 {
    if cpu.call(SMPC_LOCK_TAKE, &[]) != 0 {
        return u32::MAX;
    }
    let v = v & 0xff;
    let r = if v == 2 {
        u32::MAX
    } else {
        let v = if v > 3 { 0 } else { v };
        let mask = 3 << shift;
        let b = (cpu.read8(SMPC_MODES) as u32 & !mask) | (v << shift & mask);
        cpu.write8(SMPC_MODES, b as u8);
        cpu.leave(&[(1, !(mask as u8 as i8 as i32 as u32)), (2, SMPC_MODES)]);
        0
    };
    cpu.call(SMPC_LOCK_GIVE, &[r])
}

pub fn smpc_port1_mode(cpu: &mut Cpu, v: u32) -> u32 {
    smpc_port_mode(cpu, v, 4)
}

pub fn smpc_port2_mode(cpu: &mut Cpu, v: u32) -> u32 {
    smpc_port_mode(cpu, v, 6)
}

/// INTBACK's parameters (IREG0-IREG2) from SGL's word for them.
const INTBACK_PARAMETERS: u32 = 0x060f_ffa8;

pub fn intback_parameters(cpu: &mut Cpu) {
    let v = cpu.read32(INTBACK_PARAMETERS);
    for (k, at) in [(1, 4), (2, 2), (3, 0)] {
        cpu.write8(SMPC_IREG0 + at, (v >> (8 * k)) as u8);
    }
    cpu.leave(&[(0, v >> 24), (2, INTBACK_PARAMETERS), (3, SMPC_IREG0)]);
}

/// An SMPC command issued (`entry`, its table entry: the command, how its parameters are set):
/// once the SMPC is free (SF clear), SF set, the parameters set (the entry's third byte picks
/// SGL's setter, if any), COMREG written; waited on (SF clear again) when `wait`'s byte is set.
const SMPC_SF: u32 = 0x2010_0063;
const SMPC_COMREG: u32 = 0x2010_001f;
const SMPC_SETTERS: u32 = 0x0603_4294;

pub fn smpc_issue(cpu: &mut Cpu, entry: u32, wait: u32) -> u32 {
    let r6 = cpu.reg(6);
    while cpu.read8(SMPC_SF) & 1 != 0 {
        cpu.wait(0x0603_425e, 4);
    }
    cpu.write8(SMPC_SF, 1);
    let k = cpu.read8(entry + 2) as i8 as i32 as u32;
    let setter = cpu.read32(SMPC_SETTERS.wrapping_add(k));
    if setter != 0 {
        cpu.call(setter, &[]);
    }
    let c = cpu.read8(entry);
    cpu.write8(SMPC_COMREG, c);
    if wait & 0xff != 0 {
        while cpu.read8(SMPC_SF) & 1 != 0 {
            cpu.wait(0x0603_4286, 4);
        }
    }
    cpu.leave(&[(1, 0), (2, SMPC_COMREG), (5, wait & 0xff), (6, r6)]);
    cpu.leave_t(true);
    0
}

/// slRequestCommand: SMPC command `c` (below 17) asked for, under SGL's lock: its table entry's
/// flags say whether it may be (bit 0; bit 1 on the slave CPU, BCR1's bit 15), and its handler
/// (by the entry's second byte) makes it (`opt` handed on in r5). What the handler said, or -1.
const SMPC_REQUESTED: u32 = 0x060f_ffbc;
const SMPC_COMMAND_ENTRY: u32 = 0x0603_3ce8;
const SMPC_HANDLERS: u32 = 0x0603_4510;
const BCR1: u32 = 0xffff_ffe0;

pub fn sl_request_command(cpu: &mut Cpu, c: u32) -> u32 {
    if cpu.call(SMPC_LOCK_TAKE, &[]) != 0 {
        return u32::MAX;
    }
    let r = if c >= 17 {
        u32::MAX
    } else {
        cpu.write8(SMPC_REQUESTED, c as u8);
        let entry = cpu.call(SMPC_COMMAND_ENTRY, &[c]);
        let flags = cpu.read8(entry + 3);
        let may = flags & 1 != 0 && (cpu.read32(BCR1) & 0x8000 == 0 || flags & 2 != 0);
        if may {
            let k = cpu.read8(entry + 1) as i8 as i32 as u32;
            let h = cpu.read32(SMPC_HANDLERS.wrapping_add(k));
            cpu.call(h, &[entry])
        } else {
            u32::MAX
        }
    };
    cpu.call(SMPC_LOCK_GIVE, &[r])
}

/// The plain commands' handler: issued, unless INTBACK's data is wanted (then queued).
const SMPC_WANTED: u32 = 0x0603_3ef6;
const SMPC_QUEUE: u32 = 0x0603_3ea4;
const SMPC_ISSUE: u32 = 0x0603_4258;

pub fn smpc_request_plain(cpu: &mut Cpu, _entry: u32) -> u32 {
    if cpu.call(SMPC_WANTED, &[]) != 0 {
        cpu.call(SMPC_QUEUE, &[])
    } else {
        cpu.call(SMPC_ISSUE, &[])
    }
}

/// INTBACK's handlers: when the ports let their data be read (and, for the second, either port
/// is set), the command kept as the one going, INTBACK's parameters made (its status byte 1
/// for the first, 0 for the second; the mode byte with bit 3 set and the ports' mode bits; 0xf0)
/// and a command marked going. 0, or what the checks said.
const SMPC_GOING_COMMAND: u32 = 0x060f_ffbd;
const SMPC_PORTS_READABLE: u32 = 0x0603_3d3c;
const SMPC_ANY_PORT: u32 = 0x0603_3d7e;
const SMPC_PORT_MODES: u32 = 0x0603_3d9c;
const SMPC_MARK_GOING: u32 = 0x0603_3f2e;

fn smpc_request_intback(cpu: &mut Cpu, status: bool, any: bool) -> u32 {
    let r = cpu.call(SMPC_PORTS_READABLE, &[]);
    if r != 0 {
        return r;
    }
    if any {
        let r = cpu.call(SMPC_ANY_PORT, &[]);
        if r != 0 {
            return r;
        }
    }
    let c = cpu.read8(SMPC_REQUESTED);
    cpu.write8(SMPC_GOING_COMMAND, c);
    let modes = cpu.read8(SMPC_MODES) as u32 | if status { 0x100 } else { 0 } | 8;
    let v = cpu.call(SMPC_PORT_MODES, &[modes]);
    cpu.write32(INTBACK_PARAMETERS, ((v << 8) | 0xf0) << 8);
    cpu.call(SMPC_MARK_GOING, &[]);
    0
}

pub fn smpc_request_intback_status(cpu: &mut Cpu, _entry: u32) -> u32 {
    smpc_request_intback(cpu, true, false)
}

pub fn smpc_request_intback_pads(cpu: &mut Cpu, _entry: u32) -> u32 {
    smpc_request_intback(cpu, false, true)
}

/// SGL's peripheral records' pad words (a record each a stride apart: the device's id, then the
/// buttons held, pressed this frame and released this frame, active low): no buttons.
pub fn pad_clear(cpu: &mut Cpu, _data: u32, record: u32) {
    for k in 1..4 {
        cpu.write16(record + 2 * k, 0xffff);
    }
    cpu.leave(&[(0, u32::MAX)]);
}

/// A pad's buttons (`now`, a word) into its record: the presses and releases since the last.
pub fn pad_buttons(cpu: &mut Cpu, now: u32, record: u32) -> u32 {
    let old = cpu.read16(record + 2) as i16 as i32 as u32;
    let pressed = !(old & !now);
    let released = !(!old & now);
    cpu.write16(record + 4, pressed as u16);
    cpu.write16(record + 6, released as u16);
    cpu.write16(record + 2, now as u16);
    cpu.leave(&[(1, !now), (2, !old & now)]);
    now
}

/// The peripheral data's devices (SGL's handlers by id, from the data at `data` into the record
/// at `record`; the data after them): none (id 0xff), and a digital pad (its id, then two bytes
/// of buttons; another id than last frame's clears it instead).
const PAD_CLEAR: u32 = 0x0603_6620;
const PAD_BUTTONS: u32 = 0x0603_662a;

pub fn device_none(cpu: &mut Cpu, data: u32, record: u32) -> u32 {
    let id = cpu.read8(data);
    cpu.write8(record, id);
    cpu.call(PAD_CLEAR, &[data + 1, record]);
    data + 1
}

pub fn device_pad(cpu: &mut Cpu, data: u32, record: u32) -> u32 {
    let id = cpu.read8(data) as i8;
    let last = cpu.read8(record) as i8;
    cpu.write8(record, id as u8);
    if id != last {
        cpu.call(PAD_CLEAR, &[data + 1, record]);
        cpu.leave(&[(1, id as i32 as u32), (2, last as i32 as u32)]);
        return data + 3;
    }
    let hi = cpu.read8(data + 1) as i8 as i32 as u32;
    let lo = cpu.read8(data + 2) as u32;
    // the SH-2 code keeps the data pointer on the stack meanwhile
    cpu.push(&[data + 3]);
    cpu.call(PAD_BUTTONS, &[hi << 8 | lo, record]);
    cpu.pop(4);
    data + 3
}

/// A port's peripheral data parsed (`data_at` and `records_at` point at the data and records,
/// left after them): the port's byte (0xf0 nothing; its low half the devices; 0 with a high half
/// as one device whose byte is rewritten), then 15 devices each by SGL's handler for its id (none
/// past the port's), the records a stride apart; then the system word (the BIOS's at 0x06000728,
/// inverted) into the record at 0x060fffcc. The devices there were.
const DEVICE_KINDS: u32 = 0x0603_69d0;
const DEVICE_HANDLERS: u32 = 0x0603_6a10;
const SYSTEM_BUTTONS: u32 = 0x0600_0728;
const SYSTEM_RECORD: u32 = 0x060f_ffcc;

pub fn parse_port(cpu: &mut Cpu, data_at: u32, records_at: u32) -> u32 {
    let (r6, r7) = (cpu.reg(6), cpu.reg(7));
    let mut data = cpu.read32(data_at);
    let mut record = cpu.read32(records_at);
    let stride = cpu.read16(PERIPHERAL_STRIDE) as i16 as i32 as u32;
    let first = cpu.read8(data) as i8 as i32 as u32;
    data += 1;
    let mut devices = if first == 0xffff_fff0 { 0 } else { first & 15 };
    if first != 0xffff_fff0 && devices == 0 {
        data -= 1;
        let b = (cpu.read8(data) as i8 as i32 as u32 >> 4) | 0xffff_fff0;
        cpu.write8(data, b as u8);
        devices = 1;
    }
    let mut found = 0;
    for _ in 0..15 {
        let id = if devices == 0 {
            data -= 1;
            cpu.write8(data, 0xff);
            u32::MAX
        } else {
            let id = cpu.read8(data) as i8 as i32 as u32;
            devices -= 1;
            if id != u32::MAX {
                found += 1;
            }
            id
        };
        let kinds = cpu.read32(DEVICE_KINDS + ((id & 0xf0) >> 2));
        let k = cpu.read8(kinds + (id & 15)) as u32;
        let handler = cpu.read32(DEVICE_HANDLERS + k);
        // the SH-2 code keeps the devices left and the record on the stack meanwhile
        cpu.push(&[record, devices]);
        data = cpu.call(handler, &[data, record, stride]);
        cpu.pop(8);
        record = record.wrapping_add(stride);
    }
    cpu.write32(data_at, data);
    cpu.write32(records_at, record);
    let buttons = !(cpu.read16(SYSTEM_BUTTONS) as i16 as i32 as u32);
    cpu.call(PAD_BUTTONS, &[buttons, SYSTEM_RECORD]);
    cpu.leave(&[(1, SYSTEM_BUTTONS), (6, r6), (7, r7)]);
    found
}

/// Both ports' peripheral data parsed into SGL's records (port 2's 15 records on), unless the
/// SMPC's status says a port is not there (bits 0-1, 2-3 all set); the devices each had.
const PARSE_DATA_AT: u32 = 0x060f_ffa0;
const PARSE_RECORDS_AT: u32 = 0x060f_ffa4;
const PORT_DEVICES: u32 = 0x0603_4542;
const PARSE_PORT: u32 = 0x0603_3f90;

pub fn parse_ports(cpu: &mut Cpu) {
    let (r4, r5) = (cpu.reg(4), cpu.reg(5));
    // the SH-2 code pushes the cells' addresses around each parse
    let parse = |cpu: &mut Cpu| {
        cpu.push(&[PARSE_RECORDS_AT, PARSE_DATA_AT]);
        let found = cpu.call(PARSE_PORT, &[PARSE_DATA_AT, PARSE_RECORDS_AT]);
        cpu.pop(8);
        found
    };
    let data = cpu.read32(PERIPHERALS);
    let records = cpu.read32(PERIPHERAL_ENTRIES);
    cpu.write32(PARSE_DATA_AT, data);
    cpu.write32(PARSE_RECORDS_AT, records);
    let stride = cpu.read16(PERIPHERAL_STRIDE) as u32;
    cpu.sh2.macl = stride * 15;
    let status = cpu.read8(SMPC_KEPT) as i8 as i32 as u32;
    cpu.push(&[status]);
    let mut second = records + stride * 15;
    let mut found = 0;
    if status & 3 != 3 {
        found = parse(cpu);
        second = cpu.read32(PARSE_RECORDS_AT);
    }
    cpu.write32(PARSE_RECORDS_AT, second);
    cpu.write8(PORT_DEVICES, found as u8);
    cpu.pop(4);
    let found = if status & 12 != 12 { parse(cpu) } else { 0 };
    cpu.write8(PORT_DEVICES + 1, found as u8);
    cpu.leave(&[(3, PORT_DEVICES + 1), (4, r4), (5, r5)]);
}

/// SGL's SMPC work at the VBlank-in (an SCU mask change first, the BIOS's): under its lock, the
/// queued commands issued while the frame counter allows (SGL's queue of 32); then, if INTBACK's
/// data was wanted, no longer, and the ports parsed if it came (else none counted); then, if a
/// command is going, INTBACK's data wanted from the start and that command issued.
const SMPC_QUEUE_READ: u32 = 0x060f_ff94;
const SMPC_QUEUE_LIST: u32 = 0x0603_4544;
const FRC: u32 = 0xffff_fe12;
const PARSE_PORTS: u32 = 0x0603_4048;

fn smpc_queue_issue(cpu: &mut Cpu, frc: bool) {
    loop {
        let i = cpu.read32(SMPC_QUEUE_READ);
        let c = cpu.read8(SMPC_QUEUE_LIST.wrapping_add(i)) as i8;
        if c == -1 {
            return;
        }
        cpu.write32(SMPC_QUEUE_READ, (i + 1) & 31);
        let entry = cpu.call(SMPC_COMMAND_ENTRY, &[c as i32 as u32]);
        cpu.call(SMPC_ISSUE, &[entry]);
        if frc {
            let t = (cpu.read8(FRC) as i8 as i32) << 8 | cpu.read8(FRC + 1) as i32;
            if t >= -52 {
                return;
            }
        }
    }
}

pub fn smpc_frame_in(cpu: &mut Cpu) {
    let kept = [4, 5, 6].map(|n| (n, cpu.reg(n)));
    smpc_frame_in_work(cpu);
    cpu.leave(&kept);
}

fn smpc_frame_in_work(cpu: &mut Cpu) {
    nested!(cpu, 0x0600_8058, super::system::set_scu_mask(cpu, 0x80, 0));
    cpu.leave(&[(4, 0x80), (5, 0), (6, SMPC_QUEUE_READ)]);
    if cpu.call(SMPC_LOCK_TAKE, &[]) != 0 {
        return;
    }
    smpc_queue_issue(cpu, true);
    cpu.leave(&[(6, SMPC_STATE)]);
    let wanted = cpu.call(SMPC_WANTED, &[]);
    let came = cpu.read8(PERIPHERAL_FLAG);
    cpu.write8(PERIPHERAL_FLAG, 0);
    if wanted != 0 {
        let v = cpu.read8(SMPC_STATE) ^ 4;
        cpu.write8(SMPC_STATE, v);
        if came == 0 {
            cpu.write8(PORT_DEVICES, 0);
            cpu.write8(PORT_DEVICES + 1, 0);
            cpu.call(SMPC_LOCK_GIVE, &[]);
            return;
        }
        cpu.call(PARSE_PORTS, &[]);
    }
    if cpu.call(SMPC_GOING, &[]) != 0 {
        let v = cpu.read8(SMPC_STATE) | 4;
        cpu.write8(SMPC_STATE, v);
        cpu.write32(PERIPHERAL_FILL, 0);
        cpu.write8(INTBACK, 0);
        let c = cpu.read8(SMPC_GOING_COMMAND) as i8 as i32 as u32;
        let entry = cpu.call(SMPC_COMMAND_ENTRY, &[c]);
        cpu.call(SMPC_ISSUE, &[entry]);
    }
    cpu.call(SMPC_LOCK_GIVE, &[]);
}

/// SGL's SMPC work at the VBlank-out: under its lock, unless INTBACK's data is wanted and has
/// not come, the queued commands issued; the SMPC's busy frames counted.
const SMPC_BUSY_COUNT: u32 = 0x0603_3f68;

pub fn smpc_frame_out(cpu: &mut Cpu) {
    let kept = [4, 5, 6].map(|n| (n, cpu.reg(n)));
    smpc_frame_out_work(cpu);
    cpu.leave(&kept);
}

fn smpc_frame_out_work(cpu: &mut Cpu) {
    cpu.leave(&[(5, 0), (6, SMPC_QUEUE_READ)]);
    if cpu.call(SMPC_LOCK_TAKE, &[]) != 0 {
        return;
    }
    if cpu.call(SMPC_WANTED, &[]) == 0 || cpu.read8(PERIPHERAL_FLAG) != 0 {
        smpc_queue_issue(cpu, false);
    }
    cpu.call(SMPC_BUSY_COUNT, &[]);
    cpu.call(SMPC_LOCK_GIVE, &[]);
}

/// A copy on the CPU's DMA channel 1 (`words` words from `src` to `dst`), once it is free.
const SAR1: u32 = 0xffff_ff90;
const DAR1: u32 = 0xffff_ff94;
const TCR1: u32 = 0xffff_ff98;
const CHCR1: u32 = 0xffff_ff9c;
const DRCR1: u32 = 0xffff_fe72;

fn dma1_wait(cpu: &mut Cpu, at: u32) {
    while cpu.read32(CHCR1) & 3 == 1 {
        cpu.wait(at, 4);
    }
}

fn dma1_start(cpu: &mut Cpu, src: u32, dst: u32, words: u32) {
    cpu.read32(DMAOR);
    cpu.write32(CHCR1, 0);
    cpu.write8(DRCR1, 0);
    cpu.write32(SAR1, src);
    cpu.write32(DAR1, dst);
    cpu.write32(TCR1, words);
    cpu.write32(CHCR1, COPY_WORDS);
    cpu.write32(DMAOR, DMA_ON);
}

pub fn dma1_copy(cpu: &mut Cpu, src: u32, dst: u32, words: u32) {
    dma1_wait(cpu, 0x0603_3406);
    dma1_start(cpu, src, dst, words);
    cpu.leave(&[(0, DMA_ON), (3, SAR1)]);
}

/// SGL's VBlank-in (from the BIOS's dispatch; it saves what it uses, and runs with interrupts
/// open and GBR SGL's): the frame counter's time kept; at a sync's end (its count 0, the
/// work's flag bit 0 clear) its VDP2 register image sent to VDP2 on DMA channel 1 (and the
/// rotation parameter tables to theirs, when a rotation screen is on and they are made), waited
/// for; the VBlank transfers (slTransferEntry's) started on the SCU's level-2 DMA once its last is
/// done; slIntFunction's function, the PCM streams; at a sync's end, once 68 counts have passed,
/// the SMPC's work; the second VBlank function, the BIOS's.
const SGL_WORK: u32 = 0x060f_fc00;
const VBLANK_TIME: u32 = 0xb6;
const SYNCH_FLAGS: u32 = 0x1b;
const VDP2_REGISTERS: u32 = 0x25f8_0000;
const VDP2_IMAGE_WORDS: u32 = 0x90;
const BGON_WORD: u32 = 0xe0;
const ROTATION_SCREENS: u32 = 0x30;
const ROTATION_TABLES_VDP2: u32 = 0x214;
const ROTATION_TABLE_A: u32 = 0x060f_fe1c;
const ROTATION_TABLE_WORDS: u32 = 48;
const D2R: u32 = 0x25fe_0040;
const TRANSFER_FLAGS: u32 = 0xbd;
const INDIRECT_START: u32 = 0x0100_0007;
const INT_FUNCTION_2: u32 = 0x340;
const BIOS_VBLANK_HOOK: u32 = 0x0600_0610;
const PCM_STREAMS: u32 = 0x0603_3658;
const SMPC_FRAME_IN: u32 = 0x0603_4120;
const DMA1_COPY: u32 = 0x0603_3404;

fn frc(cpu: &mut Cpu) -> u32 {
    let h = cpu.read8(FRC) as i8 as i32 as u32;
    let l = cpu.read8(FRC + 1) as u32;
    h << 8 | l
}

pub fn vblank_in(cpu: &mut Cpu) {
    let saved = (cpu.sh2.r, cpu.sh2.gbr, cpu.sh2.mach, cpu.sh2.macl, cpu.sh2.sr, cpu.sh2.pr);
    cpu.sh2.sr = 0;
    cpu.sh2.gbr = SGL_WORK;
    let w = SGL_WORK;
    let t = frc(cpu);
    cpu.write16(w + VBLANK_TIME, t as u16);
    if cpu.read8(w + SYNCH_COUNT) == 0 && cpu.read8(w + SYNCH_FLAGS) & 1 == 0 {
        dma1_wait(cpu, 0x0603_331e);
        dma1_start(cpu, VDP2_IMAGE, VDP2_REGISTERS, VDP2_IMAGE_WORDS);
        if cpu.read16(w + BGON_WORD) as u32 & ROTATION_SCREENS != 0 && cpu.read8(w + ROTATION_BUSY) & 1 != 0 {
            let at = cpu.read32(w + ROTATION_TABLES_VDP2);
            cpu.call(DMA1_COPY, &[ROTATION_TABLE_A, at, ROTATION_TABLE_WORDS]);
            cpu.call(DMA1_COPY, &[ROTATION_TABLE_A + ROTATION_TABLE, at + 0x80, ROTATION_TABLE_WORDS]);
        }
        dma1_wait(cpu, 0x0603_3364);
    }
    if cpu.read8(w + TRANSFER_READY) != 0 {
        while cpu.read8(w + TRANSFER_FLAGS) & COPY_DONE == 0 {
            cpu.wait(0x0603_337a, 100);
        }
        let list = cpu.read32(TRANSFER_LIST);
        cpu.write32(D2R + 16, 0);
        cpu.write32(D2R + 4, list);
        cpu.write32(D2R + 12, 0x101);
        cpu.write32(D2R + 20, INDIRECT_START);
        cpu.write8(w + TRANSFER_FLAGS, 8);
        cpu.write32(D2R + 16, 0x101);
        cpu.write16(w + TRANSFER_COUNT, 0);
        cpu.write8(w + TRANSFER_READY, 0);
    }
    let f = cpu.read32(w + INT_FUNCTION);
    cpu.call(f, &[]);
    cpu.call(PCM_STREAMS, &[]);
    if cpu.read8(w + SYNCH_COUNT) == 0 {
        loop {
            let t = frc(cpu);
            let since = t.wrapping_sub(cpu.read16(w + VBLANK_TIME) as i16 as i32 as u32);
            if since >= 68 {
                break;
            }
            cpu.wait(0x0603_33cc, 140);
        }
        cpu.call(SMPC_FRAME_IN, &[]);
    }
    let f = cpu.read32(w + INT_FUNCTION_2);
    cpu.call(f, &[]);
    let f = cpu.read32(BIOS_VBLANK_HOOK);
    cpu.call(f, &[]);
    let (r, gbr, mach, macl, sr, pr) = saved;
    cpu.sh2.r[..8].copy_from_slice(&r[..8]);
    (cpu.sh2.gbr, cpu.sh2.mach, cpu.sh2.macl, cpu.sh2.sr, cpu.sh2.pr) = (gbr, mach, macl, sr, pr);
}

/// SGL's VBlank-out (from the BIOS's dispatch; it saves what it uses, and runs with interrupts
/// open and GBR SGL's): VDP1's frame change set by the screen mode and the sync (FBCR: hi-res
/// interlace alternates 8 and 12 by the field; else at a sync's last frame a change, or a manual
/// change when VDP1 has ended its frame and the slave its drawing), and where a frame begins its
/// draw started on the SCU's level-1 DMA (SGL's command list, once the last is done) or, when
/// VDP1 or the slave was not done, the sync count set to wait again (1, 2 in hi-res); then the
/// SMPC's work, the sync count stepped down, VDP2's line counter kept and the frame counted.
/// Not seen run: the hi-res path.
const VDP1_FBCR_AT: u32 = 0x25d0_0002;
const VDP1_EDSR_AT: u32 = 0x25d0_0010;
const SLAVE_DRAWN: u32 = 0x260f_fca8;
const D1R: u32 = 0x25fe_0020;
const DRAW_FLAGS: u32 = 0xbc;
const DRAW_LIST: u32 = 0x0600_89bc;
const VDP2_TVSTAT: u32 = 0x25f8_0004;
const LINE_KEPT: u32 = 0xc4;
const FRAMES: u32 = 0x22;
const SMPC_FRAME_OUT: u32 = 0x0603_41d4;

pub fn vblank_out(cpu: &mut Cpu) {
    let saved = (cpu.sh2.r, cpu.sh2.gbr, cpu.sh2.mach, cpu.sh2.macl, cpu.sh2.sr, cpu.sh2.pr);
    cpu.sh2.sr = 0;
    cpu.sh2.gbr = SGL_WORK;
    let w = SGL_WORK;
    let sb = |cpu: &mut Cpu, at: u32| cpu.read8(at) as i8 as i32 as u32;
    // the SH-2 code's places: 0x060334c4 the slave's check, 0x060334d2 the draw's start,
    // 0x06033504 the wait again, 0x06033510 the end
    enum At {
        Check,
        Draw,
        Again,
        End,
    }
    let mode = sb(cpu, w + SCREEN_MODE);
    let at = if mode & 16 != 0 {
        let flags = sb(cpu, w + SYNCH_FLAGS);
        let field = ((cpu.read16(VDP2_TVSTAT) as u32 & 2) + 4) << 1;
        cpu.write16(VDP1_FBCR_AT, field as u16);
        if flags & 2 != 0 { At::End } else { At::Check }
    } else if mode & 0x80 != 0 {
        let count = sb(cpu, w + SYNCH_COUNT);
        let flags = sb(cpu, w + SYNCH_FLAGS);
        let edsr = if count == 0 { cpu.read16(VDP1_EDSR_AT) as u32 } else { 0 };
        if count != 0 || flags & 2 != 0 {
            At::End
        } else {
            let drawn = cpu.read8(SLAVE_DRAWN);
            if edsr & CEF == 0 || drawn == 0 {
                At::Again
            } else {
                cpu.write16(VDP1_FBCR_AT, 3);
                At::Draw
            }
        }
    } else {
        let flags = sb(cpu, w + SYNCH_FLAGS);
        let frames = sb(cpu, w + SYNCH_FRAMES);
        if flags & 2 != 0 {
            At::End
        } else {
            let count = sb(cpu, w + SYNCH_COUNT);
            if frames == 1 {
                cpu.write16(VDP1_FBCR_AT, 0);
                At::Check
            } else if count == 1 {
                cpu.write16(VDP1_FBCR_AT, 2);
                At::End
            } else {
                cpu.read8(SLAVE_DRAWN);
                if count as i32 > 0 {
                    At::End
                } else {
                    cpu.write16(VDP1_FBCR_AT, 3);
                    At::Draw
                }
            }
        }
    };
    let at = match at {
        At::Check => {
            let count = sb(cpu, w + SYNCH_COUNT);
            let drawn = cpu.read8(SLAVE_DRAWN);
            if count != 0 {
                At::End
            } else if drawn == 0 {
                At::Again
            } else {
                At::Draw
            }
        }
        at => at,
    };
    match at {
        At::Draw => {
            while cpu.read8(w + DRAW_FLAGS) & COPY_DONE == 0 {
                cpu.wait(0x0603_34dc, 100);
            }
            cpu.write32(D1R + 16, 0);
            cpu.write8(w + SYNCH_A, 0);
            let list = cpu.read32(DRAW_LIST);
            cpu.write32(D1R + 4, list);
            cpu.write32(D1R + 12, 0x101);
            cpu.write32(D1R + 20, INDIRECT_START);
            cpu.write8(w + DRAW_FLAGS, 8);
            cpu.write32(D1R + 16, 0x101);
        }
        At::Again => {
            let count = ((cpu.read8(w + SCREEN_MODE) as u32 & 16) >> 4) + 1;
            cpu.write8(w + SYNCH_COUNT, count as u8);
        }
        _ => {}
    }
    cpu.call(SMPC_FRAME_OUT, &[]);
    let count = sb(cpu, w + SYNCH_COUNT).wrapping_sub(1);
    cpu.write8(w + SYNCH_COUNT, count as u8);
    let line = cpu.read16(VDP2_TVSTAT + 4);
    cpu.write16(w + LINE_KEPT, line);
    let n = cpu.read16(w + FRAMES).wrapping_add(1);
    cpu.write16(w + FRAMES, n);
    let (r, gbr, mach, macl, sr, pr) = saved;
    cpu.sh2.r[..8].copy_from_slice(&r[..8]);
    (cpu.sh2.gbr, cpu.sh2.mach, cpu.sh2.macl, cpu.sh2.sr, cpu.sh2.pr) = (gbr, mach, macl, sr, pr);
}

/// SGL's sync and interrupt set-up (slInitSystem's): the screen mode's low bits kept and bit 7
/// set for a negative `frames` (its size the frames a sync takes, at least 1, made even for a
/// hi-res `mode` (16 on)), the sync's frames and count set; SGL's handlers given the VBlank-in
/// and -out, the SCU DMA ends and the SMPC (the BIOS's interrupt setter); every DMA marked done,
/// the transfers, frame counts and flags cleared; the free-running timer's flags cleared and its
/// clock set (FTCSR, TCR); the PCM streams and sound queue emptied; the three hooks SGL's empty
/// function.
const BIOS_SET_INTERRUPT: u32 = 0x0600_0300;
const SGL_HANDLERS: [(u32, u32); 5] = [(64, 0x0603_32d6), (65, 0x0603_3430), (75, 0x0603_32a0), (74, 0x0603_32b2), (73, 0x0603_32c4)];
const SMPC_VECTOR: (u32, u32) = (71, 0x0603_4c1a);
const DMA_FLAGS_ALL: u32 = 0xb9;
const SGL_UNUSED_A: u32 = 0x354;
const SGL_UNUSED_B: u32 = 0x368;
const FTCSR: u32 = 0xffff_fe11;
const FRT_TCR: u32 = 0xffff_fe16;
const FRT_CLOCK: u8 = 2;

pub fn synch_init(cpu: &mut Cpu, mode: u32, frames: u32) {
    let w = cpu.gbr();
    let mut screen = cpu.read8(w + SCREEN_MODE) as u32 & 31;
    let mut frames = frames;
    if (frames as i32) < 0 {
        screen |= 0x80;
        frames = frames.wrapping_neg();
    }
    cpu.write8(w + SCREEN_MODE, screen as u8);
    if frames == 0 {
        frames = 1;
    }
    if mode as i32 >= 16 {
        frames = (frames + 1) & !1;
    }
    let v = (frames & 0xff) << 8 | frames.wrapping_sub(1);
    cpu.write16(w + SYNCH_FRAMES, v as u16);
    // the SH-2 code keeps r8 (the BIOS's setter) on the stack meanwhile
    let r8 = cpu.reg(8);
    cpu.push(&[r8]);
    let set = cpu.read32(BIOS_SET_INTERRUPT);
    for (vector, f) in SGL_HANDLERS {
        cpu.call(set, &[vector, f]);
    }
    for k in 0..5 {
        cpu.write8(w + DMA_FLAGS_ALL + k, 4);
    }
    cpu.write32(w + SGL_UNUSED_A, 0);
    cpu.write32(w + SGL_UNUSED_B, 0);
    cpu.call(set, &[SMPC_VECTOR.0, SMPC_VECTOR.1]);
    cpu.pop(4);
    cpu.write8(w + TRANSFER_READY, 0);
    cpu.write16(w + TRANSFER_COUNT, 0);
    cpu.write16(w + FRAMES, 0);
    cpu.write8(w + SYNCH_FLAGS, 0);
    cpu.write8(FTCSR, 0);
    cpu.write8(FRT_TCR, FRT_CLOCK);
    for k in 0..8 {
        cpu.write16(PCM_SLOTS + PCM_SLOT_SIZE * k, 0);
    }
    cpu.write16(w + PCM_CHANNELS, 0);
    cpu.write8(w + SOUND_COUNT, 0);
    for at in [INT_FUNCTION_2, INT_FUNCTION, WAIT_FUNCTION] {
        cpu.write32(w + at, EMPTY_WAIT);
    }
    cpu.leave(&[(0, EMPTY_WAIT), (1, 0), (2, PCM_SLOTS + PCM_SLOT_SIZE * 8)]);
    cpu.leave_t(true);
}

/// SGL's SMPC layer set up (slInitSystem's): its buffers' places (status, peripheral data,
/// records 24 bytes apart), its lock held while its state, flags, queue and counts are cleared
/// (the queue empty), the records emptied, the lock given back; the port settings (63, 24, the
/// bits 0), the mode bit (1) and the ports' modes (0) set; the system record cleared (-1).
const SMPC_STATUS_AT: u32 = 0x0603_4534;
const PERIPHERAL_DATA: u32 = 0x0603_4578;
const PERIPHERAL_RECORDS: u32 = 0x0603_4778;
const RECORD_STRIDE: u16 = 0x18;
const SMPC_WORDS: u32 = 0x060f_ff90;
const PERIPHERALS_CLEAR: u32 = 0x0603_3dbc;
const SMPC_INIT_CALLS: [(u32, u32); 9] = [
    (0x0603_3b68, 63),
    (0x0603_3b8e, 24),
    (0x0603_3c04, 0),
    (0x0603_3c34, 0),
    (0x0603_3c80, 0),
    (0x0603_3cb0, 0),
    (0x0603_42ba, 1),
    (0x0603_42fa, 0),
    (0x0603_4338, 0),
];

pub fn smpc_init(cpu: &mut Cpu) {
    let r4 = cpu.reg(4);
    cpu.write32(SMPC_STATUS_AT, SMPC_STATUS);
    cpu.write32(PERIPHERALS, PERIPHERAL_DATA);
    cpu.write32(PERIPHERAL_ENTRIES, PERIPHERAL_RECORDS);
    cpu.write16(PERIPHERAL_STRIDE, RECORD_STRIDE);
    cpu.write8(SMPC_LOCK, 0x80);
    cpu.write8(PORT_DEVICES, 0);
    for k in 1..13 {
        cpu.write8(SMPC_LOCK + k, 0);
    }
    cpu.write8(PORT_DEVICES + 1, 0);
    for k in 0..10 {
        cpu.write32(SMPC_WORDS + 4 * k, 0);
    }
    cpu.write8(SMPC_QUEUE_LIST, 0xff);
    cpu.call(PERIPHERALS_CLEAR, &[]);
    cpu.call(SMPC_LOCK_GIVE, &[]);
    for (f, v) in SMPC_INIT_CALLS {
        cpu.call(f, &[v]);
    }
    for k in 0..6 {
        cpu.write32(SYSTEM_RECORD + 4 * k, u32::MAX);
    }
    cpu.leave(&[(0, u32::MAX), (1, SYSTEM_RECORD), (4, r4)]);
}

/// The slave's command handlers (SGL's, on CPU 1; its command loop hands each the command's
/// first word in r0 and in r4 the place after it, and goes on from where r4 is left). The
/// slave's work fields share SGL's work area (GBR the same): the window it draws through and its
/// settings (WINDOWS: each window's fields a word or long 4 apart), the light, the polygon
/// buffers it takes.
const SLAVE_WINDOW: u32 = 0xa9;
const SLAVE_LEVEL: u32 = 0xad;
const SLAVE_DEPTH: u32 = 0x6c;
const SLAVE_LIGHT: u32 = 0x58;
const SLAVE_SETTINGS: [u32; 3] = [0x37c, 0x384, 0x38c];
const W_LEVEL_BYTE: u32 = 0x1f;
/// The windows' settings as the slave addresses them (through the cache; the master's WINDOWS
/// is the same RAM read around it).
const SLAVE_WINDOWS: u32 = 0x060f_fefc;

fn slave_window_field(cpu: &mut Cpu, field: u32) -> u32 {
    let w = cpu.gbr();
    let k = (cpu.read8(w + SLAVE_WINDOW) as i8 as i32 as u32 & 3) << 2;
    SLAVE_WINDOWS + k + field
}

/// 48: the function in the first word called with the next word (the slave's registers kept).
pub fn slave_call(cpu: &mut Cpu) {
    let (f, at) = (cpu.reg(0), cpu.reg(4));
    let arg = cpu.read32(at);
    cpu.call(f, &[arg]);
    cpu.leave(&[(4, at + 4)]);
}

/// 52: the near limit's level for the slave's window.
pub fn slave_zdsp_level(cpu: &mut Cpu) {
    let level = cpu.reg(0);
    let w = cpu.gbr();
    cpu.write8(w + SLAVE_LEVEL, level as u8);
    let at = slave_window_field(cpu, W_LEVEL_BYTE);
    cpu.write8(at, level as u8);
    cpu.leave(&[(0, at - SLAVE_WINDOWS), (1, level), (3, at)]);
}

/// 12: the window depth (slPerspective's) for the slave's window.
pub fn slave_perspective(cpu: &mut Cpu) {
    let depth = cpu.reg(0);
    let w = cpu.gbr();
    cpu.write32(w + SLAVE_DEPTH, depth);
    let at = slave_window_field(cpu, W_DEPTH);
    cpu.write32(at, depth);
    cpu.leave(&[(0, at - SLAVE_WINDOWS), (1, depth), (3, SLAVE_WINDOWS)]);
}

/// 36: the light (three words).
pub fn slave_light(cpu: &mut Cpu) {
    let (x, at) = (cpu.reg(0), cpu.reg(4));
    let w = cpu.gbr();
    cpu.write32(w + SLAVE_LIGHT, x);
    let y = cpu.read32(at);
    cpu.write32(w + SLAVE_LIGHT + 4, y);
    let z = cpu.read32(at + 4);
    cpu.write32(w + SLAVE_LIGHT + 8, z);
    cpu.leave(&[(0, z), (4, at + 8)]);
}

/// 60, 64, 68: the settings the master sends (slave_setting_60's and _68's), kept.
fn slave_setting_set(cpu: &mut Cpu, k: usize) {
    let v = cpu.reg(0);
    let w = cpu.gbr();
    cpu.write32(w + SLAVE_SETTINGS[k], v);
}

pub fn slave_keep_60(cpu: &mut Cpu) {
    slave_setting_set(cpu, 0);
}

pub fn slave_keep_64(cpu: &mut Cpu) {
    slave_setting_set(cpu, 1);
}

pub fn slave_keep_68(cpu: &mut Cpu) {
    slave_setting_set(cpu, 2);
}

/// 56: the window (0 or 1) the slave draws through: its polygon buffer (window 0's the second
/// half), level, depth, far limit, size and screen place taken from its settings.
const SLAVE_BUFFER_AREA: u32 = 0x0600_89c0;
const SLAVE_BUFFER: u32 = 0x38;
const SLAVE_FAR: u32 = 0xa0;
const SLAVE_SIZE: u32 = 0x330;
const SLAVE_WIDTH: u32 = 0x94;
const SLAVE_SCREEN: u32 = 0x9c;
const SLAVE_SCREEN_CENTRE: u32 = 0x260f_ff04;

fn slave_window_set(cpu: &mut Cpu, win: u32) {
    let w = cpu.gbr();
    let b = (cpu.read8(w + SLAVE_WINDOW) as i8 as i32 as u32 & 0xf0) | win;
    cpu.write8(w + SLAVE_WINDOW, b as u8);
    let s = SLAVE_WINDOWS.wrapping_add(win << 2);
    let mut buffer = cpu.read32(SLAVE_BUFFER_AREA);
    if win << 2 == 0 {
        buffer = buffer.wrapping_add(0x200);
    }
    cpu.write32(w + SLAVE_BUFFER, buffer);
    let level = cpu.read16(s + W_FLAG);
    cpu.write8(w + SLAVE_LEVEL, level as u8);
    let depth = cpu.read32(s + W_DEPTH);
    cpu.write32(w + SLAVE_DEPTH, depth);
    let far = cpu.read16(s + W_CENTRE);
    cpu.write16(w + SLAVE_FAR, far);
    let (top, left) = (cpu.read32(s + W_TOP), cpu.read32(s + W_LEFT));
    let right = cpu.read32(s + W_RIGHT);
    cpu.write32(w + SLAVE_SIZE, top.wrapping_add(0x1_0001).wrapping_sub(left));
    let span = right.wrapping_sub(left);
    cpu.write32(w + SLAVE_WIDTH, span);
    let centre = cpu.read32(SLAVE_SCREEN_CENTRE);
    let high = span.rotate_left(16).wrapping_add(centre);
    let v = (high << 16) | (centre.wrapping_add(span << 16) >> 16);
    cpu.write32(w + SLAVE_SCREEN, v);
    cpu.leave(&[(0, v), (1, span << 16), (2, high), (3, left)]);
}

pub fn slave_window(cpu: &mut Cpu) {
    let win = cpu.reg(0);
    slave_window_set(cpu, win);
}

/// 44: the frame's end (it has no word after it: r4 stepped back): the points room and polygon
/// buffer back to SGL's areas (the buffer's half by the master's side), the cache purged, and
/// window 0.
const SLAVE_POINTS: u32 = 0x28;
const SLAVE_POLYGONS: u32 = 0x34;

pub fn slave_frame_end(cpu: &mut Cpu) {
    let w = cpu.gbr();
    let at = cpu.reg(4) - 4;
    let points = cpu.read32(POINT_AREA) + POINT_ROOM_SIZE;
    cpu.write32(w + SLAVE_POINTS, points);
    let area = cpu.read32(POLYGON_AREA);
    let side = cpu.read32(w + BUFFER_SIDE);
    cpu.write32(w + SLAVE_POLYGONS, side.wrapping_add(area));
    cpu.leave(&[(1, area), (4, at)]);
    nested!(cpu, 0x0603_64a0, super::system::purge_cache(cpu));
    cpu.write8(w + SLAVE_WINDOW, 0);
    slave_window_set(cpu, 0);
}

/// The slave's drawing: commands built in its polygon buffer (36 bytes each: control word,
/// colours, texture size, places, Gouraud table, depth, and the next command in its depth's
/// list) and linked into its depth table (a list per 256 depth units, from [GBR+0x38]); the
/// window's draw bit marked (bit 4 window 0, bit 5 window 1).
const SLAVE_SLOT: u32 = 0x34;
const SLAVE_DEPTHS: u32 = 0x38;
const SLAVE_TEXTURES: u32 = 0x3c;
const SLAVE_FAR_LIMIT: u32 = 0xa0;
const SLAVE_SCREEN_X: u32 = 0x9c;
const SLAVE_SCREEN_Y: u32 = 0x9e;
const SLAVE_RIGHT: u32 = 0x8c;
const SLAVE_BOTTOM: u32 = 0x8e;
const ENTRY_DEPTH: u32 = 30;
const ENTRY_NEXT: u32 = 32;

fn slave_mark_drawn(cpu: &mut Cpu) -> u32 {
    let w = cpu.gbr();
    let b = cpu.read8(w + SLAVE_WINDOW) as i8 as i32 as u32;
    let bit = if b & 1 != 0 { 32 } else { 16 };
    cpu.write8(w + SLAVE_WINDOW, (b | bit) as u8);
    bit
}

fn slave_link(cpu: &mut Cpu, entry: u32, depth_word: u32) {
    let w = cpu.gbr();
    let table = cpu.read32(w + SLAVE_DEPTHS);
    let at = table.wrapping_add((depth_word >> 8) << 2);
    let head = cpu.read32(at);
    cpu.write32(at, entry);
    cpu.write32(entry + ENTRY_NEXT, head);
}

/// 40: the master's ready-made commands (sl_put_quad's, put_polygons'): the first word the last
/// of them (they go down 36 bytes each), then their count and the points they used (the points
/// room moved on past them); each linked by its depth.
pub fn slave_quads(cpu: &mut Cpu) {
    let w = cpu.gbr();
    let (mut entry, at) = (cpu.reg(0), cpu.reg(4));
    let count = cpu.read32(at);
    let points = cpu.read32(at + 4);
    if count as i32 > 0 {
        let room = cpu.read32(w + SLAVE_POINTS).wrapping_add(points << 4);
        cpu.write32(w + SLAVE_POINTS, room);
        for _ in 0..count {
            let depth = cpu.read16(entry + ENTRY_DEPTH) as i16 as i32 as u32;
            slave_link(cpu, entry, depth);
            entry = entry.wrapping_sub(POLYGON_ENTRY);
        }
    }
    let bit = slave_mark_drawn(cpu);
    let table = cpu.read32(w + SLAVE_DEPTHS);
    // SGL's code counts down in r9, which its loop does not keep
    let r9 = if count as i32 > 0 { 0 } else { count };
    cpu.leave(&[(1, bit), (2, table), (4, at + 8), (6, entry), (9, r9)]);
}

/// SWAP.B: the low half's two bytes swapped.
fn swap_b(v: u32) -> u32 {
    (v & 0xffff_0000) | (v & 0xff) << 8 | (v >> 8 & 0xff)
}

/// A sprite command's start (0x06037eb0, which returns past its caller to skip one): the first
/// word's low half its depth (past the far limit: none, r4 past the command's `rest` bytes), its
/// high half the control word; the command begun at the slave's slot and linked by its depth,
/// its colour and Gouraud table, and the texture's address from SGL's table with its size (times
/// the command's scale word, on MACL). The entry, the control word, the scaled width and height.
fn slave_sprite_start(cpu: &mut Cpu, rest: u32) -> Option<(u32, u32, u32, u32)> {
    let w = cpu.gbr();
    let (arg, at) = (cpu.reg(0), cpu.reg(4));
    let depth = arg & 0xffff;
    let control = arg.rotate_left(16);
    let far = cpu.read16(w + SLAVE_FAR_LIMIT) as i16 as i32 as u32;
    if depth > far {
        cpu.leave(&[(1, depth), (4, at + rest), (5, control)]);
        return None;
    }
    let entry = cpu.read32(w + SLAVE_SLOT);
    cpu.write16(entry, control as u16);
    slave_mark_drawn(cpu);
    cpu.write16(entry + ENTRY_DEPTH, depth as u16);
    slave_link(cpu, entry, depth);
    let gouraud = cpu.read16(at + 2);
    cpu.write16(entry + 28, gouraud);
    let colours = cpu.read32(at + 4);
    let texno = cpu.read16(at) as u32;
    cpu.write32(entry + 4, colours);
    let texture = cpu.read32(w + SLAVE_TEXTURES).wrapping_add(texno << 3);
    let hsize = cpu.read16(texture) as i16 as i32 as u32;
    let scale = cpu.read32(at + 16);
    let vsize = cpu.read16(texture + 2) as i16 as i32 as u32;
    let address = cpu.read32(texture + 4);
    cpu.write32(entry + 8, address);
    let (width, height) = (hsize.wrapping_mul(scale), vsize.wrapping_mul(scale));
    cpu.sh2.macl = height;
    cpu.leave(&[(4, at - 4), (11, entry)]);
    Some((entry, control, width, height))
}

/// 28: a scaled sprite (the master's slPutSprite's): its place (x, y words) and size, unless off
/// the window's right or bottom (then its link undone); its command a scaled sprite, its mode's
/// bits made up (high-speed shrink and the like) where the master left them clear; the slot moved on.
pub fn slave_sprite(cpu: &mut Cpu) {
    let w = cpu.gbr();
    let at = cpu.reg(4);
    let Some((entry, control, width, height)) = slave_sprite_start(cpu, 20) else {
        return;
    };
    let base = at - 4;
    let x = cpu.read16(base + 12) as i16 as i32 as u32;
    let place = cpu.read32(base + 16);
    let right = x.wrapping_add(cpu.read16(w + SLAVE_SCREEN_X) as i16 as i32 as u32);
    let width = width >> 16;
    cpu.leave(&[(4, base + 24)]);
    let mut off = right > cpu.read16(w + SLAVE_RIGHT) as i16 as i32 as u32;
    let size = (width << 16) | (height >> 16);
    let xy = (x << 16) | (place >> 16);
    if !off {
        let y = (place.rotate_left(16) as u16 as i16 as i32 as u32).wrapping_add(cpu.read16(w + SLAVE_SCREEN_Y) as i16 as i32 as u32);
        off = y > cpu.read16(w + SLAVE_BOTTOM) as i16 as i32 as u32;
    }
    if off {
        let depth = cpu.read16(entry + ENTRY_DEPTH) as u32;
        let next = cpu.read32(entry + ENTRY_NEXT);
        let table = cpu.read32(w + SLAVE_DEPTHS);
        cpu.write32(table + (((depth >> 8) & 0xff) << 2), next);
        return;
    }
    cpu.write32(entry + 12, xy);
    cpu.write32(entry + 16, size);
    let mut c = swap_b((control & 0xffff_fff0) + 1);
    if c & 3 == 0 {
        c |= 2;
        if c & 12 == 0 {
            c |= 8;
        }
    }
    let c = swap_b(c);
    cpu.write16(entry, c as u16);
    cpu.write32(w + SLAVE_SLOT, entry + POLYGON_ENTRY);
    cpu.leave(&[(0, entry + POLYGON_ENTRY), (1, xy), (3, size), (5, (control & 0xffff_fff0) + 1), (7, x)]);
}

/// 24: a turned sprite (the master's slPutSprite with a turn): begun as a sprite is (its texture's
/// size scaled), then its four corners from its place turned by the command's sine and cosine
/// words, in the order its flip bits pick (a jump table: the control word's bits set to match),
/// each checked against the window's bounds (one off: the link undone); then, when the sprite
/// crosses the screen's edge, its corners turned round to start from the nearest (the distorted
/// sprite's clipping). SGL's code block by block (tools/port/asm2sm.py): it leaves r8-r14 as it
/// used them.
pub fn slave_turned_sprite(cpu: &mut Cpu) {
    let Some((entry, control, width, height)) = slave_sprite_start(cpu, 28) else {
        return;
    };
    cpu.leave(&[(2, width), (3, height), (5, control), (11, entry)]);
    turned_sprite_rest(cpu);
}

#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
fn turned_sprite_rest(cpu: &mut Cpu) {
    let w = cpu.gbr();
    let [mut r0, mut r1, mut r2, mut r3, mut r4, mut r5, mut r6, mut r7, mut r8, mut r9, mut r10, mut r11, mut r12, mut r13, _, mut r15] = cpu.sh2.r;
    let mut r14 = cpu.caller_r14;
    let mut mac = ((cpu.sh2.mach as u64) << 32 | cpu.sh2.macl as u64) as i64;
    let mut t = cpu.sh2.sr & 1 != 0;
    let entry_pr = cpu.sh2.pr;
    let mut pr = entry_pr;
    macro_rules! hand_over { () => { cpu.sh2.r = [r0, r1, r2, r3, r4, r5, r6, r7, r8, r9, r10, r11, r12, r13, r14, r15]; cpu.leave_mac(mac); cpu.leave_t(t); } }
    macro_rules! take_back { () => { [r0, r1, r2, r3, r4, r5, r6, r7, r8, r9, r10, r11, r12, r13, r14, r15] = cpu.sh2.r; mac = ((cpu.sh2.mach as u64) << 32 | cpu.sh2.macl as u64) as i64; t = cpu.sh2.sr & 1 != 0; } }
    let mut pc: u32 = 0x06037f7a;
    loop {
        match pc {
            0x06037f7a => {
                r7 = cpu.read32(r4.wrapping_add(24)); // 06037f7a mov.l @(24,r4),r7
                pr = cpu.read32(r15); r15 = r15.wrapping_add(4); // 06037f7c lds.l @r15+,pr
                mac = r2 as i32 as i64 * r7 as i32 as i64; // 06037f7e dmuls.l r2,r7
                r0 = cpu.read32(w.wrapping_add(140)); // 06037f80 mov.l @(140,gbr),r0
                r8 = cpu.read32(r4.wrapping_add(28)); // 06037f82 mov.l @(28,r4),r8
                r14 = r0; // 06037f84 mov r0,r14
                r1 = (mac >> 32) as u32; // 06037f86 sts mach,r1
                r6 = mac as u32; // 06037f88 sts macl,r6
                mac = r3 as i32 as i64 * r7 as i32 as i64; // 06037f8a dmuls.l r3,r7
                r14 <<= 16; // 06037f8c shll16 r14
                r0 >>= 16; // 06037f8e shlr16 r0
                r6 = (r1 << 16) | (r6 >> 16); // 06037f90 xtrct r1,r6
                r1 = (mac >> 32) as u32; // 06037f92 sts mach,r1
                r7 = mac as u32; // 06037f94 sts macl,r7
                mac = r2 as i32 as i64 * r8 as i32 as i64; // 06037f96 dmuls.l r2,r8
                r13 = r0.rotate_left(16); // 06037f98 swap.w r0,r13
                r0 = cpu.read32(w.wrapping_add(156)); // 06037f9a mov.l @(156,gbr),r0
                r7 = (r1 << 16) | (r7 >> 16); // 06037f9c xtrct r1,r7
                r1 = (mac >> 32) as u32; // 06037f9e sts mach,r1
                r2 = mac as u32; // 06037fa0 sts macl,r2
                mac = r3 as i32 as i64 * r8 as i32 as i64; // 06037fa2 dmuls.l r3,r8
                r12 = r0; // 06037fa4 mov r0,r12
                r12 <<= 16; // 06037fa6 shll16 r12
                r2 = (r1 << 16) | (r2 >> 16); // 06037fa8 xtrct r1,r2
                r1 = (mac >> 32) as u32; // 06037faa sts mach,r1
                r3 = mac as u32; // 06037fac sts macl,r3
                r0 >>= 16; // 06037fae shlr16 r0
                r10 = r0.rotate_left(16); // 06037fb0 swap.w r0,r10
                r3 = (r1 << 16) | (r3 >> 16); // 06037fb2 xtrct r1,r3
                r0 = (r5 & 0xffff_0000) | ((r5 & 0xff) << 8) | ((r5 >> 8) & 0xff); // 06037fb4 swap.b r5,r0
                r0 &= 15; // 06037fb6 and #15,r0
                r1 = r0; // 06037fb8 mov r0,r1
                r1 <<= 2; // 06037fba shll2 r1
                r0 = 0x06037fc8; // 06037fbc mova 0x6037fc8,r0
                r1 = cpu.read32(r0.wrapping_add(r1)); // 06037fbe mov.l @(r0,r1),r1
                r8 = cpu.read32(r4.wrapping_add(12)); // 06037fc0 mov.l @(12,r4),r8
                let to = r1; // 06037fc2 jmp @r1
                r9 = cpu.read32(r4.wrapping_add(16)); // 06037fc4 mov.l @(16,r4),r9
                pc = to;
            }
            0x06038008 => {
                r1 = 32i32 as u32; // 06038008 mov #32,r1
                r5 ^= r1; // 0603800a xor r1,r5
                cpu.write16(r11, r5 as u16); // 0603800c mov.w r5,@r11
                r3 = r3.wrapping_neg(); // 0603800e neg r3,r3
                r7 = r7.wrapping_neg(); // 06038010 neg r7,r7
                pc = 0x06038012;
            }
            0x06038012 => {
                r0 = r2; // 06038012 mov r2,r0
                t = r0 & 1 != 0; r0 = (r0 as i32 >> 1) as u32; // 06038014 shar r0
                r8 = r8.wrapping_sub(r0); // 06038016 sub r0,r8
                r0 = r8; // 06038018 mov r8,r0
                r0 = r0.wrapping_add(r10); // 0603801a add r10,r0
                t = r0 > r13; // 0603801c cmp/hi r13,r0
                let take = t; // 0603801e bt/s 0x603807e
                r4 = r4.wrapping_add(32i32 as u32); // 06038020 add #32,r4
                pc = if take { 0x0603807e } else { 0x06038022 };
            }
            0x06038022 => {
                r0 = r6; // 06038022 mov r6,r0
                t = r0 & 1 != 0; r0 = (r0 as i32 >> 1) as u32; // 06038024 shar r0
                r9 = r9.wrapping_sub(r0); // 06038026 sub r0,r9
                r0 = r9; // 06038028 mov r9,r0
                r0 = r0.wrapping_add(r12); // 0603802a add r12,r0
                t = r0 > r14; // 0603802c cmp/hi r14,r0
                let take = t; // 0603802e bt/s 0x603807e
                r1 = r8.rotate_left(16); // 06038030 swap.w r8,r1
                pc = if take { 0x0603807e } else { 0x06038032 };
            }
            0x06038032 => {
                r8 = r8.wrapping_add(r2); // 06038032 add r2,r8
                r0 = r8; // 06038034 mov r8,r0
                r0 = r0.wrapping_add(r10); // 06038036 add r10,r0
                t = r0 > r13; // 06038038 cmp/hi r13,r0
                let take = t; // 0603803a bt/s 0x603807e
                r5 = r9; // 0603803c mov r9,r5
                pc = if take { 0x0603807e } else { 0x0603803e };
            }
            0x0603803e => {
                r9 = r9.wrapping_add(r6); // 0603803e add r6,r9
                r0 = r9; // 06038040 mov r9,r0
                r0 = r0.wrapping_add(r12); // 06038042 add r12,r0
                t = r0 > r14; // 06038044 cmp/hi r14,r0
                let take = t; // 06038046 bt/s 0x603807e
                r0 = r8.rotate_left(16); // 06038048 swap.w r8,r0
                pc = if take { 0x0603807e } else { 0x0603804a };
            }
            0x0603804a => {
                r5 = (r1 << 16) | (r5 >> 16); // 0603804a xtrct r1,r5
                r1 = r9; // 0603804c mov r9,r1
                r1 = (r0 << 16) | (r1 >> 16); // 0603804e xtrct r0,r1
                r8 = r8.wrapping_sub(r7); // 06038050 sub r7,r8
                r0 = r8; // 06038052 mov r8,r0
                r0 = r0.wrapping_add(r10); // 06038054 add r10,r0
                t = r0 > r13; // 06038056 cmp/hi r13,r0
                let take = t; // 06038058 bt/s 0x603807e
                r9 = r9.wrapping_add(r3); // 0603805a add r3,r9
                pc = if take { 0x0603807e } else { 0x0603805c };
            }
            0x0603805c => {
                r0 = r9; // 0603805c mov r9,r0
                r0 = r0.wrapping_add(r12); // 0603805e add r12,r0
                t = r0 > r14; // 06038060 cmp/hi r14,r0
                let take = t; // 06038062 bt/s 0x603807e
                r3 = r8.rotate_left(16); // 06038064 swap.w r8,r3
                pc = if take { 0x0603807e } else { 0x06038066 };
            }
            0x06038066 => {
                r8 = r8.wrapping_sub(r2); // 06038066 sub r2,r8
                r10 = r10.wrapping_add(r8); // 06038068 add r8,r10
                t = r10 > r13; // 0603806a cmp/hi r13,r10
                let take = t; // 0603806c bt/s 0x603807e
                r7 = r9; // 0603806e mov r9,r7
                pc = if take { 0x0603807e } else { 0x06038070 };
            }
            0x06038070 => {
                r9 = r9.wrapping_sub(r6); // 06038070 sub r6,r9
                r12 = r12.wrapping_add(r9); // 06038072 add r9,r12
                t = r12 > r14; // 06038074 cmp/hi r14,r12
                let take = t; // 06038076 bt/s 0x603807e
                r7 = (r3 << 16) | (r7 >> 16); // 06038078 xtrct r3,r7
                pc = if take { 0x0603807e } else { 0x0603807a };
            }
            0x0603807a => {
                // 0603807a bra 0x6038210
                r8 = r8.rotate_left(16); // 0603807c swap.w r8,r8
                pc = 0x06038210;
            }
            0x0603807e => {
                r0 = cpu.read16(r11.wrapping_add(30)) as i16 as i32 as u32; // 0603807e mov.w @(30,r11),r0
                r1 = cpu.read32(r11.wrapping_add(32)); // 06038080 mov.l @(32,r11),r1
                r2 = (r0 & 0xffff_0000) | ((r0 & 0xff) << 8) | ((r0 >> 8) & 0xff); // 06038082 swap.b r0,r2
                r0 = cpu.read32(w.wrapping_add(56)); // 06038084 mov.l @(56,gbr),r0
                r2 = r2 & 0xff; // 06038086 extu.b r2,r2
                r2 <<= 2; // 06038088 shll2 r2
                // 0603808a rts
                cpu.write32(r0.wrapping_add(r2), r1); // 0603808c mov.l r1,@(r0,r2)
                if pr == entry_pr { break; }
                pc = pr;
            }
            0x0603808e => {
                r1 = 32i32 as u32; // 0603808e mov #32,r1
                r3 = r3.wrapping_neg(); // 06038090 neg r3,r3
                // 06038092 bra 0x60380a4
                r7 = r7.wrapping_neg(); // 06038094 neg r7,r7
                pc = 0x060380a4;
            }
            0x06038096 => {
                r1 = 48i32 as u32; // 06038096 mov #48,r1
                r3 = r3.wrapping_neg(); // 06038098 neg r3,r3
                // 0603809a bra 0x60380a0
                r7 = r7.wrapping_neg(); // 0603809c neg r7,r7
                pc = 0x060380a0;
            }
            0x0603809e => {
                r1 = 16i32 as u32; // 0603809e mov #16,r1
                pc = 0x060380a0;
            }
            0x060380a0 => {
                r2 = r2.wrapping_neg(); // 060380a0 neg r2,r2
                r6 = r6.wrapping_neg(); // 060380a2 neg r6,r6
                pc = 0x060380a4;
            }
            0x060380a4 => {
                r5 ^= r1; // 060380a4 xor r1,r5
                cpu.write16(r11, r5 as u16); // 060380a6 mov.w r5,@r11
                pc = 0x060380a8;
            }
            0x060380a8 => {
                r0 = r8; // 060380a8 mov r8,r0
                r0 = r0.wrapping_add(r10); // 060380aa add r10,r0
                t = r0 > r13; // 060380ac cmp/hi r13,r0
                let take = t; // 060380ae bt/s 0x603807e
                r4 = r4.wrapping_add(32i32 as u32); // 060380b0 add #32,r4
                pc = if take { 0x0603807e } else { 0x060380b2 };
            }
            0x060380b2 => {
                r0 = r9; // 060380b2 mov r9,r0
                r0 = r0.wrapping_add(r12); // 060380b4 add r12,r0
                t = r0 > r14; // 060380b6 cmp/hi r14,r0
                let take = t; // 060380b8 bt/s 0x603807e
                r0 = r8.rotate_left(16); // 060380ba swap.w r8,r0
                pc = if take { 0x0603807e } else { 0x060380bc };
            }
            0x060380bc => {
                r5 = r9; // 060380bc mov r9,r5
                r5 = (r0 << 16) | (r5 >> 16); // 060380be xtrct r0,r5
                r8 = r8.wrapping_add(r2); // 060380c0 add r2,r8
                r0 = r8; // 060380c2 mov r8,r0
                r0 = r0.wrapping_add(r10); // 060380c4 add r10,r0
                t = r0 > r13; // 060380c6 cmp/hi r13,r0
                let take = t; // 060380c8 bt/s 0x603807e
                r9 = r9.wrapping_add(r6); // 060380ca add r6,r9
                pc = if take { 0x0603807e } else { 0x060380cc };
            }
            0x060380cc => {
                r0 = r9; // 060380cc mov r9,r0
                r0 = r0.wrapping_add(r12); // 060380ce add r12,r0
                t = r0 > r14; // 060380d0 cmp/hi r14,r0
                let take = t; // 060380d2 bt/s 0x603807e
                r0 = r8.rotate_left(16); // 060380d4 swap.w r8,r0
                pc = if take { 0x0603807e } else { 0x060380d6 };
            }
            0x060380d6 => {
                r1 = r9; // 060380d6 mov r9,r1
                r1 = (r0 << 16) | (r1 >> 16); // 060380d8 xtrct r0,r1
                r8 = r8.wrapping_sub(r7); // 060380da sub r7,r8
                r0 = r8; // 060380dc mov r8,r0
                r0 = r0.wrapping_add(r10); // 060380de add r10,r0
                t = r0 > r13; // 060380e0 cmp/hi r13,r0
                let take = t; // 060380e2 bt/s 0x603807e
                r9 = r9.wrapping_add(r3); // 060380e4 add r3,r9
                pc = if take { 0x0603807e } else { 0x060380e6 };
            }
            0x060380e6 => {
                r0 = r9; // 060380e6 mov r9,r0
                r0 = r0.wrapping_add(r12); // 060380e8 add r12,r0
                t = r0 > r14; // 060380ea cmp/hi r14,r0
                let take = t; // 060380ec bt/s 0x603807e
                r3 = r8.rotate_left(16); // 060380ee swap.w r8,r3
                pc = if take { 0x0603807e } else { 0x060380f0 };
            }
            0x060380f0 => {
                r8 = r8.wrapping_sub(r2); // 060380f0 sub r2,r8
                r10 = r10.wrapping_add(r8); // 060380f2 add r8,r10
                t = r10 > r13; // 060380f4 cmp/hi r13,r10
                let take = t; // 060380f6 bt/s 0x603807e
                r7 = r9; // 060380f8 mov r9,r7
                pc = if take { 0x0603807e } else { 0x060380fa };
            }
            0x060380fa => {
                r9 = r9.wrapping_sub(r6); // 060380fa sub r6,r9
                r12 = r12.wrapping_add(r9); // 060380fc add r9,r12
                t = r12 > r14; // 060380fe cmp/hi r14,r12
                let take = t; // 06038100 bt/s 0x603807e
                r7 = (r3 << 16) | (r7 >> 16); // 06038102 xtrct r3,r7
                pc = if take { 0x0603807e } else { 0x06038104 };
            }
            0x06038104 => {
                // 06038104 bra 0x6038210
                r8 = r8.rotate_left(16); // 06038106 swap.w r8,r8
                pc = 0x06038210;
            }
            0x06038108 => {
                r1 = 16i32 as u32; // 06038108 mov #16,r1
                r5 ^= r1; // 0603810a xor r1,r5
                cpu.write16(r11, r5 as u16); // 0603810c mov.w r5,@r11
                r2 = r2.wrapping_neg(); // 0603810e neg r2,r2
                r6 = r6.wrapping_neg(); // 06038110 neg r6,r6
                pc = 0x06038112;
            }
            0x06038112 => {
                r0 = r7; // 06038112 mov r7,r0
                t = r0 & 1 != 0; r0 = (r0 as i32 >> 1) as u32; // 06038114 shar r0
                r8 = r8.wrapping_add(r0); // 06038116 add r0,r8
                r0 = r8; // 06038118 mov r8,r0
                r0 = r0.wrapping_add(r10); // 0603811a add r10,r0
                t = r0 > r13; // 0603811c cmp/hi r13,r0
                let take = t; // 0603811e bt/s 0x603817e
                r4 = r4.wrapping_add(32i32 as u32); // 06038120 add #32,r4
                pc = if take { 0x0603817e } else { 0x06038122 };
            }
            0x06038122 => {
                r0 = r3; // 06038122 mov r3,r0
                t = r0 & 1 != 0; r0 = (r0 as i32 >> 1) as u32; // 06038124 shar r0
                r9 = r9.wrapping_sub(r0); // 06038126 sub r0,r9
                r0 = r9; // 06038128 mov r9,r0
                r0 = r0.wrapping_add(r12); // 0603812a add r12,r0
                t = r0 > r14; // 0603812c cmp/hi r14,r0
                let take = t; // 0603812e bt/s 0x603817e
                r1 = r8.rotate_left(16); // 06038130 swap.w r8,r1
                pc = if take { 0x0603817e } else { 0x06038132 };
            }
            0x06038132 => {
                r8 = r8.wrapping_add(r2); // 06038132 add r2,r8
                r0 = r8; // 06038134 mov r8,r0
                r0 = r0.wrapping_add(r10); // 06038136 add r10,r0
                t = r0 > r13; // 06038138 cmp/hi r13,r0
                let take = t; // 0603813a bt/s 0x603817e
                r5 = r9; // 0603813c mov r9,r5
                pc = if take { 0x0603817e } else { 0x0603813e };
            }
            0x0603813e => {
                r9 = r9.wrapping_add(r6); // 0603813e add r6,r9
                r0 = r9; // 06038140 mov r9,r0
                r0 = r0.wrapping_add(r12); // 06038142 add r12,r0
                t = r0 > r14; // 06038144 cmp/hi r14,r0
                let take = t; // 06038146 bt/s 0x603817e
                r0 = r8.rotate_left(16); // 06038148 swap.w r8,r0
                pc = if take { 0x0603817e } else { 0x0603814a };
            }
            0x0603814a => {
                r5 = (r1 << 16) | (r5 >> 16); // 0603814a xtrct r1,r5
                r1 = r9; // 0603814c mov r9,r1
                r1 = (r0 << 16) | (r1 >> 16); // 0603814e xtrct r0,r1
                r8 = r8.wrapping_sub(r7); // 06038150 sub r7,r8
                r0 = r8; // 06038152 mov r8,r0
                r0 = r0.wrapping_add(r10); // 06038154 add r10,r0
                t = r0 > r13; // 06038156 cmp/hi r13,r0
                let take = t; // 06038158 bt/s 0x603817e
                r9 = r9.wrapping_add(r3); // 0603815a add r3,r9
                pc = if take { 0x0603817e } else { 0x0603815c };
            }
            0x0603815c => {
                r0 = r9; // 0603815c mov r9,r0
                r0 = r0.wrapping_add(r12); // 0603815e add r12,r0
                t = r0 > r14; // 06038160 cmp/hi r14,r0
                let take = t; // 06038162 bt/s 0x603817e
                r3 = r8.rotate_left(16); // 06038164 swap.w r8,r3
                pc = if take { 0x0603817e } else { 0x06038166 };
            }
            0x06038166 => {
                r8 = r8.wrapping_sub(r2); // 06038166 sub r2,r8
                r10 = r10.wrapping_add(r8); // 06038168 add r8,r10
                t = r10 > r13; // 0603816a cmp/hi r13,r10
                let take = t; // 0603816c bt/s 0x603817e
                r7 = r9; // 0603816e mov r9,r7
                pc = if take { 0x0603817e } else { 0x06038170 };
            }
            0x06038170 => {
                r9 = r9.wrapping_sub(r6); // 06038170 sub r6,r9
                r12 = r12.wrapping_add(r9); // 06038172 add r9,r12
                t = r12 > r14; // 06038174 cmp/hi r14,r12
                let take = t; // 06038176 bt/s 0x603817e
                r7 = (r3 << 16) | (r7 >> 16); // 06038178 xtrct r3,r7
                pc = if take { 0x0603817e } else { 0x0603817a };
            }
            0x0603817a => {
                // 0603817a bra 0x6038210
                r8 = r8.rotate_left(16); // 0603817c swap.w r8,r8
                pc = 0x06038210;
            }
            0x0603817e => {
                r0 = cpu.read16(r11.wrapping_add(30)) as i16 as i32 as u32; // 0603817e mov.w @(30,r11),r0
                r1 = cpu.read32(r11.wrapping_add(32)); // 06038180 mov.l @(32,r11),r1
                r2 = (r0 & 0xffff_0000) | ((r0 & 0xff) << 8) | ((r0 >> 8) & 0xff); // 06038182 swap.b r0,r2
                r0 = cpu.read32(w.wrapping_add(56)); // 06038184 mov.l @(56,gbr),r0
                r2 = r2 & 0xff; // 06038186 extu.b r2,r2
                r2 <<= 2; // 06038188 shll2 r2
                // 0603818a rts
                cpu.write32(r0.wrapping_add(r2), r1); // 0603818c mov.l r1,@(r0,r2)
                if pr == entry_pr { break; }
                pc = pr;
            }
            0x0603818e => {
                r5 = (r5 & 0xffff_0000) | ((r5 & 0xff) << 8) | ((r5 >> 8) & 0xff); // 0603818e swap.b r5,r5
                r0 = -16i32 as u32; // 06038190 mov #-16,r0
                r0 &= r5; // 06038192 and r5,r0
                r0 |= 10; // 06038194 or #10,r0
                r0 = (r0 & 0xffff_0000) | ((r0 & 0xff) << 8) | ((r0 >> 8) & 0xff); // 06038196 swap.b r0,r0
                cpu.write16(r11.wrapping_add(0), r0 as u16); // 06038198 mov.w r0,@(0,r11)
                pc = 0x0603819a;
            }
            0x0603819a => {
                r0 = r2; // 0603819a mov r2,r0
                t = r0 & 1 != 0; r0 = (r0 as i32 >> 1) as u32; // 0603819c shar r0
                r8 = r8.wrapping_sub(r0); // 0603819e sub r0,r8
                r0 = r7; // 060381a0 mov r7,r0
                t = r0 & 1 != 0; r0 = (r0 as i32 >> 1) as u32; // 060381a2 shar r0
                r8 = r8.wrapping_add(r0); // 060381a4 add r0,r8
                r0 = r8; // 060381a6 mov r8,r0
                r0 = r0.wrapping_add(r10); // 060381a8 add r10,r0
                t = r0 > r13; // 060381aa cmp/hi r13,r0
                let take = t; // 060381ac bt/s 0x603817e
                r4 = r4.wrapping_add(32i32 as u32); // 060381ae add #32,r4
                pc = if take { 0x0603817e } else { 0x060381b0 };
            }
            0x060381b0 => {
                r0 = r3; // 060381b0 mov r3,r0
                t = r0 & 1 != 0; r0 = (r0 as i32 >> 1) as u32; // 060381b2 shar r0
                r9 = r9.wrapping_sub(r0); // 060381b4 sub r0,r9
                r0 = r6; // 060381b6 mov r6,r0
                t = r0 & 1 != 0; r0 = (r0 as i32 >> 1) as u32; // 060381b8 shar r0
                r9 = r9.wrapping_sub(r0); // 060381ba sub r0,r9
                r0 = r9; // 060381bc mov r9,r0
                r0 = r0.wrapping_add(r12); // 060381be add r12,r0
                t = r0 > r14; // 060381c0 cmp/hi r14,r0
                let take = t; // 060381c2 bt/s 0x603817e
                r1 = r8.rotate_left(16); // 060381c4 swap.w r8,r1
                pc = if take { 0x0603817e } else { 0x060381c6 };
            }
            0x060381c6 => {
                r8 = r8.wrapping_add(r2); // 060381c6 add r2,r8
                r0 = r8; // 060381c8 mov r8,r0
                r0 = r0.wrapping_add(r10); // 060381ca add r10,r0
                t = r0 > r13; // 060381cc cmp/hi r13,r0
                let take = t; // 060381ce bt/s 0x603817e
                r5 = r9; // 060381d0 mov r9,r5
                pc = if take { 0x0603817e } else { 0x060381d2 };
            }
            0x060381d2 => {
                r5 = (r1 << 16) | (r5 >> 16); // 060381d2 xtrct r1,r5
                r9 = r9.wrapping_add(r6); // 060381d4 add r6,r9
                r0 = r9; // 060381d6 mov r9,r0
                r0 = r0.wrapping_add(r12); // 060381d8 add r12,r0
                t = r0 > r14; // 060381da cmp/hi r14,r0
                let take = t; // 060381dc bt/s 0x603817e
                r0 = r8.rotate_left(16); // 060381de swap.w r8,r0
                pc = if take { 0x0603817e } else { 0x060381e0 };
            }
            0x060381e0 => {
                r1 = r9; // 060381e0 mov r9,r1
                r1 = (r0 << 16) | (r1 >> 16); // 060381e2 xtrct r0,r1
                r8 = r8.wrapping_sub(r7); // 060381e4 sub r7,r8
                r0 = r8; // 060381e6 mov r8,r0
                r0 = r0.wrapping_add(r10); // 060381e8 add r10,r0
                t = r0 > r13; // 060381ea cmp/hi r13,r0
                let take = t; // 060381ec bt/s 0x603817e
                r9 = r9.wrapping_add(r3); // 060381ee add r3,r9
                pc = if take { 0x0603817e } else { 0x060381f0 };
            }
            0x060381f0 => {
                r0 = r9; // 060381f0 mov r9,r0
                r0 = r0.wrapping_add(r12); // 060381f2 add r12,r0
                t = r0 > r14; // 060381f4 cmp/hi r14,r0
                let take = t; // 060381f6 bt/s 0x603817e
                r3 = r8.rotate_left(16); // 060381f8 swap.w r8,r3
                pc = if take { 0x0603817e } else { 0x060381fa };
            }
            0x060381fa => {
                r8 = r8.wrapping_sub(r2); // 060381fa sub r2,r8
                r10 = r10.wrapping_add(r8); // 060381fc add r8,r10
                t = r10 > r13; // 060381fe cmp/hi r13,r10
                let take = t; // 06038200 bt/s 0x603817e
                r7 = r9; // 06038202 mov r9,r7
                pc = if take { 0x0603817e } else { 0x06038204 };
            }
            0x06038204 => {
                r9 = r9.wrapping_sub(r6); // 06038204 sub r6,r9
                r12 = r12.wrapping_add(r9); // 06038206 add r9,r12
                t = r12 > r14; // 06038208 cmp/hi r14,r12
                let take = t; // 0603820a bt/s 0x603817e
                r7 = (r3 << 16) | (r7 >> 16); // 0603820c xtrct r3,r7
                pc = if take { 0x0603817e } else { 0x0603820e };
            }
            0x0603820e => {
                r8 = r8.rotate_left(16); // 0603820e swap.w r8,r8
                pc = 0x06038210;
            }
            0x06038210 => {
                r0 = r11; // 06038210 mov r11,r0
                r0 = r0.wrapping_add(36i32 as u32); // 06038212 add #36,r0
                cpu.write32(w.wrapping_add(52), r0); // 06038214 mov.l r0,@(52,gbr)
                r0 = cpu.read32(w.wrapping_add(148)); // 06038216 mov.l @(148,gbr),r0
                r3 = r0 as i16 as i32 as u32; // 06038218 exts.w r0,r3
                r2 = r0.rotate_left(16); // 0603821a swap.w r0,r2
                r0 = cpu.read32(w.wrapping_add(816)); // 0603821c mov.l @(816,gbr),r0
                r2 = r2 as i16 as i32 as u32; // 0603821e exts.w r2,r2
                r13 = r0 & 0xffff; // 06038220 extu.w r0,r13
                r12 = r0.rotate_left(16); // 06038222 swap.w r0,r12
                r12 = r12 & 0xffff; // 06038224 extu.w r12,r12
                r0 = cpu.read16(r11.wrapping_add(4)) as i16 as i32 as u32; // 06038226 mov.w @(4,r11),r0
                r0 &= 4; // 06038228 and #4,r0
                t = r0 & r0 == 0; // 0603822a tst r0,r0
                r9 = (r8 << 16) | (r9 >> 16); // 0603822c xtrct r8,r9
                pc = if !t { 0x060382ee } else { 0x06038230 }; // 0603822e bf 0x60382ee
            }
            0x06038230 => {
                r0 = r5 as i16 as i32 as u32; // 06038230 exts.w r5,r0
                r0 = r0.wrapping_add(r3); // 06038232 add r3,r0
                t = r0 >= r13; // 06038234 cmp/hs r13,r0
                let take = t; // 06038236 bt/s 0x6038242
                r0 = r5.rotate_left(16); // 06038238 swap.w r5,r0
                pc = if take { 0x06038242 } else { 0x0603823a };
            }
            0x0603823a => {
                r0 = r0 as i16 as i32 as u32; // 0603823a exts.w r0,r0
                r0 = r0.wrapping_add(r2); // 0603823c add r2,r0
                t = r12 > r0; // 0603823e cmp/hi r0,r12
                let take = t; // 06038240 bt/s 0x60382ee
                r0 = r1 as i16 as i32 as u32; // 06038242 exts.w r1,r0
                pc = if take { 0x060382ee } else { 0x06038244 };
            }
            0x06038242 => {
                r0 = r1 as i16 as i32 as u32; // 06038242 exts.w r1,r0
                pc = 0x06038244;
            }
            0x06038244 => {
                r0 = r0.wrapping_add(r3); // 06038244 add r3,r0
                t = r0 >= r13; // 06038246 cmp/hs r13,r0
                let take = t; // 06038248 bt/s 0x6038254
                r0 = r1.rotate_left(16); // 0603824a swap.w r1,r0
                pc = if take { 0x06038254 } else { 0x0603824c };
            }
            0x0603824c => {
                r0 = r0 as i16 as i32 as u32; // 0603824c exts.w r0,r0
                r0 = r0.wrapping_add(r2); // 0603824e add r2,r0
                t = r12 > r0; // 06038250 cmp/hi r0,r12
                let take = t; // 06038252 bt/s 0x60382f8
                r0 = r7 as i16 as i32 as u32; // 06038254 exts.w r7,r0
                pc = if take { 0x060382f8 } else { 0x06038256 };
            }
            0x06038254 => {
                r0 = r7 as i16 as i32 as u32; // 06038254 exts.w r7,r0
                pc = 0x06038256;
            }
            0x06038256 => {
                r0 = r0.wrapping_add(r3); // 06038256 add r3,r0
                t = r0 >= r13; // 06038258 cmp/hs r13,r0
                let take = t; // 0603825a bt/s 0x6038266
                r0 = r7.rotate_left(16); // 0603825c swap.w r7,r0
                pc = if take { 0x06038266 } else { 0x0603825e };
            }
            0x0603825e => {
                r0 = r0 as i16 as i32 as u32; // 0603825e exts.w r0,r0
                r0 = r0.wrapping_add(r2); // 06038260 add r2,r0
                t = r12 > r0; // 06038262 cmp/hi r0,r12
                let take = t; // 06038264 bt/s 0x6038308
                r0 = r9 as i16 as i32 as u32; // 06038266 exts.w r9,r0
                pc = if take { 0x06038308 } else { 0x06038268 };
            }
            0x06038266 => {
                r0 = r9 as i16 as i32 as u32; // 06038266 exts.w r9,r0
                pc = 0x06038268;
            }
            0x06038268 => {
                r0 = r0.wrapping_add(r3); // 06038268 add r3,r0
                t = r0 >= r13; // 0603826a cmp/hs r13,r0
                let take = t; // 0603826c bt/s 0x6038276
                r0 = r8 as i16 as i32 as u32; // 0603826e exts.w r8,r0
                pc = if take { 0x06038276 } else { 0x06038270 };
            }
            0x06038270 => {
                r0 = r0.wrapping_add(r2); // 06038270 add r2,r0
                t = r12 > r0; // 06038272 cmp/hi r0,r12
                let take = t; // 06038274 bt/s 0x6038318
                r8 = 1i32 as u32; // 06038276 mov #1,r8
                pc = if take { 0x06038318 } else { 0x06038278 };
            }
            0x06038276 => {
                r8 = 1i32 as u32; // 06038276 mov #1,r8
                pc = 0x06038278;
            }
            0x06038278 => {
                r8 <<= 16; // 06038278 shll16 r8
                t = r8 & 1 != 0; r8 >>= 1; // 0603827a shlr r8
                r2 = r9.rotate_left(16); // 0603827c swap.w r9,r2
                t = r8 & r2 == 0; // 0603827e tst r8,r2
                { let (x, b1) = r13.overflowing_sub(r13); let (y, b2) = x.overflowing_sub(t as u32); r13 = y; t = b1 || b2; } // 06038280 subc r13,r13
                r13 = !r13; // 06038282 not r13,r13
                r13 ^= r2; // 06038284 xor r2,r13
                t = r8 & r9 == 0; // 06038286 tst r8,r9
                { let (x, b1) = r2.overflowing_sub(r2); let (y, b2) = x.overflowing_sub(t as u32); r2 = y; t = b1 || b2; } // 06038288 subc r2,r2
                r2 = !r2; // 0603828a not r2,r2
                r2 ^= r9; // 0603828c xor r9,r2
                r13 = r13.wrapping_add(r2); // 0603828e add r2,r13
                r13 = r13 & 0xffff; // 06038290 extu.w r13,r13
                r0 = 0x06038318; // 06038292 mova 0x6038318,r0
                r2 = r7.rotate_left(16); // 06038294 swap.w r7,r2
                t = r8 & r2 == 0; // 06038296 tst r8,r2
                { let (x, b1) = r12.overflowing_sub(r12); let (y, b2) = x.overflowing_sub(t as u32); r12 = y; t = b1 || b2; } // 06038298 subc r12,r12
                r12 = !r12; // 0603829a not r12,r12
                r12 ^= r2; // 0603829c xor r2,r12
                t = r8 & r7 == 0; // 0603829e tst r8,r7
                { let (x, b1) = r2.overflowing_sub(r2); let (y, b2) = x.overflowing_sub(t as u32); r2 = y; t = b1 || b2; } // 060382a0 subc r2,r2
                r2 = !r2; // 060382a2 not r2,r2
                r2 ^= r7; // 060382a4 xor r7,r2
                r12 = r12.wrapping_add(r2); // 060382a6 add r2,r12
                r12 = r12 & 0xffff; // 060382a8 extu.w r12,r12
                t = r12 as i32 > r13 as i32; // 060382aa cmp/gt r13,r12
                let take = t; // 060382ac bt/s 0x60382b4
                r2 = r1.rotate_left(16); // 060382ae swap.w r1,r2
                pc = if take { 0x060382b4 } else { 0x060382b0 };
            }
            0x060382b0 => {
                r13 = r12; // 060382b0 mov r12,r13
                r0 = 0x06038308; // 060382b2 mova 0x6038308,r0
                pc = 0x060382b4;
            }
            0x060382b4 => {
                t = r8 & r2 == 0; // 060382b4 tst r8,r2
                { let (x, b1) = r12.overflowing_sub(r12); let (y, b2) = x.overflowing_sub(t as u32); r12 = y; t = b1 || b2; } // 060382b6 subc r12,r12
                r12 = !r12; // 060382b8 not r12,r12
                r12 ^= r2; // 060382ba xor r2,r12
                t = r8 & r1 == 0; // 060382bc tst r8,r1
                { let (x, b1) = r2.overflowing_sub(r2); let (y, b2) = x.overflowing_sub(t as u32); r2 = y; t = b1 || b2; } // 060382be subc r2,r2
                r2 = !r2; // 060382c0 not r2,r2
                r2 ^= r1; // 060382c2 xor r1,r2
                r12 = r12.wrapping_add(r2); // 060382c4 add r2,r12
                r12 = r12 & 0xffff; // 060382c6 extu.w r12,r12
                t = r12 as i32 > r13 as i32; // 060382c8 cmp/gt r13,r12
                let take = t; // 060382ca bt/s 0x60382d2
                r2 = r5.rotate_left(16); // 060382cc swap.w r5,r2
                pc = if take { 0x060382d2 } else { 0x060382ce };
            }
            0x060382ce => {
                r13 = r12; // 060382ce mov r12,r13
                r0 = 0x06038308; // 060382d0 mova 0x6038308,r0
                pc = 0x060382d2;
            }
            0x060382d2 => {
                t = r8 & r2 == 0; // 060382d2 tst r8,r2
                { let (x, b1) = r12.overflowing_sub(r12); let (y, b2) = x.overflowing_sub(t as u32); r12 = y; t = b1 || b2; } // 060382d4 subc r12,r12
                r12 = !r12; // 060382d6 not r12,r12
                r12 ^= r2; // 060382d8 xor r2,r12
                t = r8 & r5 == 0; // 060382da tst r8,r5
                { let (x, b1) = r2.overflowing_sub(r2); let (y, b2) = x.overflowing_sub(t as u32); r2 = y; t = b1 || b2; } // 060382dc subc r2,r2
                r2 = !r2; // 060382de not r2,r2
                r2 ^= r5; // 060382e0 xor r5,r2
                r12 = r12.wrapping_add(r2); // 060382e2 add r2,r12
                r12 = r12 & 0xffff; // 060382e4 extu.w r12,r12
                t = r13 as i32 >= r12 as i32; // 060382e6 cmp/ge r12,r13
                pc = if t { 0x060382ee } else { 0x060382ea }; // 060382e8 bt 0x60382ee
            }
            0x060382ea => {
                let to = r0; // 060382ea jmp @r0
                // nop // 060382ec nop 
                pc = to;
            }
            0x060382ee => {
                cpu.write32(r11.wrapping_add(12), r5); // 060382ee mov.l r5,@(12,r11)
                cpu.write32(r11.wrapping_add(16), r1); // 060382f0 mov.l r1,@(16,r11)
                cpu.write32(r11.wrapping_add(20), r7); // 060382f2 mov.l r7,@(20,r11)
                // 060382f4 rts
                cpu.write32(r11.wrapping_add(24), r9); // 060382f6 mov.l r9,@(24,r11)
                if pr == entry_pr { break; }
                pc = pr;
            }
            0x060382f8 => {
                r0 = cpu.read16(r11.wrapping_add(0)) as i16 as i32 as u32; // 060382f8 mov.w @(0,r11),r0
                cpu.write32(r11.wrapping_add(16), r5); // 060382fa mov.l r5,@(16,r11)
                r0 ^= 16; // 060382fc xor #16,r0
                cpu.write16(r11.wrapping_add(0), r0 as u16); // 060382fe mov.w r0,@(0,r11)
                cpu.write32(r11.wrapping_add(12), r1); // 06038300 mov.l r1,@(12,r11)
                cpu.write32(r11.wrapping_add(24), r7); // 06038302 mov.l r7,@(24,r11)
                // 06038304 rts
                cpu.write32(r11.wrapping_add(20), r9); // 06038306 mov.l r9,@(20,r11)
                if pr == entry_pr { break; }
                pc = pr;
            }
            0x06038308 => {
                r0 = cpu.read16(r11.wrapping_add(0)) as i16 as i32 as u32; // 06038308 mov.w @(0,r11),r0
                cpu.write32(r11.wrapping_add(20), r5); // 0603830a mov.l r5,@(20,r11)
                r0 ^= 48; // 0603830c xor #48,r0
                cpu.write16(r11.wrapping_add(0), r0 as u16); // 0603830e mov.w r0,@(0,r11)
                cpu.write32(r11.wrapping_add(24), r1); // 06038310 mov.l r1,@(24,r11)
                cpu.write32(r11.wrapping_add(12), r7); // 06038312 mov.l r7,@(12,r11)
                // 06038314 rts
                cpu.write32(r11.wrapping_add(16), r9); // 06038316 mov.l r9,@(16,r11)
                if pr == entry_pr { break; }
                pc = pr;
            }
            0x06038318 => {
                r0 = cpu.read16(r11.wrapping_add(0)) as i16 as i32 as u32; // 06038318 mov.w @(0,r11),r0
                cpu.write32(r11.wrapping_add(24), r5); // 0603831a mov.l r5,@(24,r11)
                r0 ^= 32; // 0603831c xor #32,r0
                cpu.write16(r11.wrapping_add(0), r0 as u16); // 0603831e mov.w r0,@(0,r11)
                cpu.write32(r11.wrapping_add(20), r1); // 06038320 mov.l r1,@(20,r11)
                cpu.write32(r11.wrapping_add(16), r7); // 06038322 mov.l r7,@(16,r11)
                // 06038324 rts
                cpu.write32(r11.wrapping_add(12), r9); // 06038326 mov.l r9,@(12,r11)
                if pr == entry_pr { break; }
                pc = pr;
            }
            _ => unreachable!("{pc:#010x}"),
        }
    }
    hand_over!();
    cpu.leave_r14(r14);
}

/// 20: polygon data (the master's slPutPolygon: the counts, the points, the polygons, their
/// attributes, then the matrix): the matrix kept (SGL's copy in its code), the points turned and
/// put on the screen into the points room (each with its clipping bits against the window, the
/// window's near level and the far limit; the divide unit's quotients for the perspective), then
/// each polygon facing the eye (or two-sided) given a depth by its attribute's sort, its
/// lighting or Gouraud table where its attribute asks, and its VDP1 command made at the slave's
/// slot and linked by its depth (a chain of handlers per attribute kind, through SGL's tables).
/// SGL's code block by block (tools/port/asm2sm.py: about 2000 instructions in 227 blocks); it
/// makes its own pushes (PROLOGUES) and leaves r8-r14 as it used them.
pub fn slave_polygons(cpu: &mut Cpu) {
    slave_polygons_blocks(cpu);
}

#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
fn slave_polygons_blocks(cpu: &mut Cpu) {
    let w = cpu.gbr();
    let [mut r0, mut r1, mut r2, mut r3, mut r4, mut r5, mut r6, mut r7, mut r8, mut r9, mut r10, mut r11, mut r12, mut r13, _, mut r15] = cpu.sh2.r;
    let mut r14 = cpu.caller_r14;
    let mut mac = ((cpu.sh2.mach as u64) << 32 | cpu.sh2.macl as u64) as i64;
    let mut t = cpu.sh2.sr & 1 != 0;
    let entry_pr = cpu.sh2.pr;
    let mut pr = entry_pr;
    macro_rules! hand_over { () => { cpu.sh2.r = [r0, r1, r2, r3, r4, r5, r6, r7, r8, r9, r10, r11, r12, r13, r14, r15]; cpu.leave_mac(mac); cpu.leave_t(t); } }
    macro_rules! take_back { () => { [r0, r1, r2, r3, r4, r5, r6, r7, r8, r9, r10, r11, r12, r13, r14, r15] = cpu.sh2.r; mac = ((cpu.sh2.mach as u64) << 32 | cpu.sh2.macl as u64) as i64; t = cpu.sh2.sr & 1 != 0; } }
    let mut pc: u32 = 0x06036dd8;
    loop {
        match pc {
            0x06036c40 => {
                r0 = 0x00000200; // 06036c40 mov.l 0x6036f58,r0
                r1 = 0i32 as u32; // 06036c42 mov #0,r1
                pc = 0x06036c44;
            }
            0x06036c44 => {
                t = r3 as i32 > r0 as i32; // 06036c44 cmp/gt r0,r3
                pc = if !t { 0x06036c50 } else { 0x06036c48 }; // 06036c46 bf 0x6036c50
            }
            0x06036c48 => {
                r1 = 1i32 as u32; // 06036c48 mov #1,r1
                t = r3 & 1 != 0; r3 = (r3 as i32 >> 1) as u32; // 06036c4a shar r3
                // 06036c4c bra 0x6036c44
                t = r5 & 1 != 0; r5 = (r5 as i32 >> 1) as u32; // 06036c4e shar r5
                pc = 0x06036c44;
            }
            0x06036c50 => {
                t = r5 as i32 > r0 as i32; // 06036c50 cmp/gt r0,r5
                pc = if !t { 0x06036c5c } else { 0x06036c54 }; // 06036c52 bf 0x6036c5c
            }
            0x06036c54 => {
                r1 = 1i32 as u32; // 06036c54 mov #1,r1
                t = r3 & 1 != 0; r3 = (r3 as i32 >> 1) as u32; // 06036c56 shar r3
                // 06036c58 bra 0x6036c50
                t = r5 & 1 != 0; r5 = (r5 as i32 >> 1) as u32; // 06036c5a shar r5
                pc = 0x06036c50;
            }
            0x06036c5c => {
                r0 = r0.wrapping_neg(); // 06036c5c neg r0,r0
                t = r3 as i32 >= r0 as i32; // 06036c5e cmp/ge r0,r3
                pc = if t { 0x06036c6a } else { 0x06036c62 }; // 06036c60 bt 0x6036c6a
            }
            0x06036c62 => {
                r1 = 1i32 as u32; // 06036c62 mov #1,r1
                t = r3 & 1 != 0; r3 = (r3 as i32 >> 1) as u32; // 06036c64 shar r3
                // 06036c66 bra 0x6036c5c
                t = r5 & 1 != 0; r5 = (r5 as i32 >> 1) as u32; // 06036c68 shar r5
                pc = 0x06036c5c;
            }
            0x06036c6a => {
                t = r5 as i32 >= r0 as i32; // 06036c6a cmp/ge r0,r5
                pc = if t { 0x06036c76 } else { 0x06036c6e }; // 06036c6c bt 0x6036c76
            }
            0x06036c6e => {
                r1 = 1i32 as u32; // 06036c6e mov #1,r1
                t = r3 & 1 != 0; r3 = (r3 as i32 >> 1) as u32; // 06036c70 shar r3
                // 06036c72 bra 0x6036c6a
                t = r5 & 1 != 0; r5 = (r5 as i32 >> 1) as u32; // 06036c74 shar r5
                pc = 0x06036c6a;
            }
            0x06036c76 => {
                t = r1 & r1 == 0; // 06036c76 tst r1,r1
                pc = if !t { 0x06036c7e } else { 0x06036c7a }; // 06036c78 bf 0x6036c7e
            }
            0x06036c7a => {
                // 06036c7a bra 0x6036ee0
                // nop // 06036c7c nop 
                pc = 0x06036ee0;
            }
            0x06036c7e => {
                r0 = r3; // 06036c7e mov r3,r0
                r1 = r5; // 06036c80 mov r5,r1
                r4 = r3; // 06036c82 mov r3,r4
                r7 = r5; // 06036c84 mov r5,r7
                t = r0 as i32 >= 0; // 06036c86 cmp/pz r0
                pc = if t { 0x06036c8c } else { 0x06036c8a }; // 06036c88 bt 0x6036c8c
            }
            0x06036c8a => {
                r0 = r0.wrapping_neg(); // 06036c8a neg r0,r0
                pc = 0x06036c8c;
            }
            0x06036c8c => {
                t = r1 as i32 >= 0; // 06036c8c cmp/pz r1
                pc = if t { 0x06036c92 } else { 0x06036c90 }; // 06036c8e bt 0x6036c92
            }
            0x06036c90 => {
                r1 = r1.wrapping_neg(); // 06036c90 neg r1,r1
                pc = 0x06036c92;
            }
            0x06036c92 => {
                t = r1 as i32 >= r0 as i32; // 06036c92 cmp/ge r0,r1
                let take = t; // 06036c94 bt/s 0x6036cd2
                r0 = 4i32 as u32; // 06036c96 mov #4,r0
                pc = if take { 0x06036cd2 } else { 0x06036c98 };
            }
            0x06036c98 => {
                t = r3 as i32 >= 0; // 06036c98 cmp/pz r3
                pc = if !t { 0x06036cb6 } else { 0x06036c9c }; // 06036c9a bf 0x6036cb6
            }
            0x06036c9c => {
                r1 = 0x00000200; // 06036c9c mov.l 0x6036f58,r1
                pc = 0x06036c9e;
            }
            0x06036c9e => {
                r3 = r3.wrapping_add(r4); // 06036c9e add r4,r3
                t = r3 as i32 > r1 as i32; // 06036ca0 cmp/gt r1,r3
                let take = t; // 06036ca2 bt/s 0x6036caa
                r3 = r3.wrapping_sub(r4); // 06036ca4 sub r4,r3
                pc = if take { 0x06036caa } else { 0x06036ca6 };
            }
            0x06036ca6 => {
                r5 = r5.wrapping_add(r7); // 06036ca6 add r7,r5
                r3 = r3.wrapping_add(r4); // 06036ca8 add r4,r3
                pc = 0x06036caa;
            }
            0x06036caa => {
                t = r7 & 1 != 0; r7 = (r7 as i32 >> 1) as u32; // 06036caa shar r7
                r0 = r0.wrapping_sub(1); t = r0 == 0; // 06036cac dt r0
                let take = !t; // 06036cae bf/s 0x6036c9e
                t = r4 & 1 != 0; r4 = (r4 as i32 >> 1) as u32; // 06036cb0 shar r4
                pc = if take { 0x06036c9e } else { 0x06036cb2 };
            }
            0x06036cb2 => {
                // 06036cb2 bra 0x6036ee0
                // nop // 06036cb4 nop 
                pc = 0x06036ee0;
            }
            0x06036cb6 => {
                r1 = 0x00000200; // 06036cb6 mov.l 0x6036f58,r1
                r1 = r1.wrapping_neg(); // 06036cb8 neg r1,r1
                pc = 0x06036cba;
            }
            0x06036cba => {
                r3 = r3.wrapping_add(r4); // 06036cba add r4,r3
                t = r3 as i32 >= r1 as i32; // 06036cbc cmp/ge r1,r3
                let take = !t; // 06036cbe bf/s 0x6036cc6
                r3 = r3.wrapping_sub(r4); // 06036cc0 sub r4,r3
                pc = if take { 0x06036cc6 } else { 0x06036cc2 };
            }
            0x06036cc2 => {
                r5 = r5.wrapping_add(r7); // 06036cc2 add r7,r5
                r3 = r3.wrapping_add(r4); // 06036cc4 add r4,r3
                pc = 0x06036cc6;
            }
            0x06036cc6 => {
                t = r7 & 1 != 0; r7 = (r7 as i32 >> 1) as u32; // 06036cc6 shar r7
                r0 = r0.wrapping_sub(1); t = r0 == 0; // 06036cc8 dt r0
                let take = !t; // 06036cca bf/s 0x6036cba
                t = r4 & 1 != 0; r4 = (r4 as i32 >> 1) as u32; // 06036ccc shar r4
                pc = if take { 0x06036cba } else { 0x06036cce };
            }
            0x06036cce => {
                // 06036cce bra 0x6036ee0
                // nop // 06036cd0 nop 
                pc = 0x06036ee0;
            }
            0x06036cd2 => {
                t = r5 as i32 >= 0; // 06036cd2 cmp/pz r5
                pc = if !t { 0x06036cf0 } else { 0x06036cd6 }; // 06036cd4 bf 0x6036cf0
            }
            0x06036cd6 => {
                r1 = 0x00000200; // 06036cd6 mov.l 0x6036f58,r1
                pc = 0x06036cd8;
            }
            0x06036cd8 => {
                r5 = r5.wrapping_add(r7); // 06036cd8 add r7,r5
                t = r5 as i32 > r1 as i32; // 06036cda cmp/gt r1,r5
                let take = t; // 06036cdc bt/s 0x6036ce4
                r5 = r5.wrapping_sub(r7); // 06036cde sub r7,r5
                pc = if take { 0x06036ce4 } else { 0x06036ce0 };
            }
            0x06036ce0 => {
                r3 = r3.wrapping_add(r4); // 06036ce0 add r4,r3
                r5 = r5.wrapping_add(r7); // 06036ce2 add r7,r5
                pc = 0x06036ce4;
            }
            0x06036ce4 => {
                t = r4 & 1 != 0; r4 = (r4 as i32 >> 1) as u32; // 06036ce4 shar r4
                r0 = r0.wrapping_sub(1); t = r0 == 0; // 06036ce6 dt r0
                let take = !t; // 06036ce8 bf/s 0x6036cd8
                t = r7 & 1 != 0; r7 = (r7 as i32 >> 1) as u32; // 06036cea shar r7
                pc = if take { 0x06036cd8 } else { 0x06036cec };
            }
            0x06036cec => {
                // 06036cec bra 0x6036ee0
                // nop // 06036cee nop 
                pc = 0x06036ee0;
            }
            0x06036cf0 => {
                r1 = 0x00000200; // 06036cf0 mov.l 0x6036f58,r1
                r1 = r1.wrapping_neg(); // 06036cf2 neg r1,r1
                pc = 0x06036cf4;
            }
            0x06036cf4 => {
                r5 = r5.wrapping_add(r7); // 06036cf4 add r7,r5
                t = r5 as i32 >= r1 as i32; // 06036cf6 cmp/ge r1,r5
                let take = !t; // 06036cf8 bf/s 0x6036d00
                r5 = r5.wrapping_sub(r7); // 06036cfa sub r7,r5
                pc = if take { 0x06036d00 } else { 0x06036cfc };
            }
            0x06036cfc => {
                r3 = r3.wrapping_add(r4); // 06036cfc add r4,r3
                r5 = r5.wrapping_add(r7); // 06036cfe add r7,r5
                pc = 0x06036d00;
            }
            0x06036d00 => {
                t = r4 & 1 != 0; r4 = (r4 as i32 >> 1) as u32; // 06036d00 shar r4
                r0 = r0.wrapping_sub(1); t = r0 == 0; // 06036d02 dt r0
                let take = !t; // 06036d04 bf/s 0x6036cf4
                t = r7 & 1 != 0; r7 = (r7 as i32 >> 1) as u32; // 06036d06 shar r7
                pc = if take { 0x06036cf4 } else { 0x06036d08 };
            }
            0x06036d08 => {
                // 06036d08 bra 0x6036ee0
                // nop // 06036d0a nop 
                pc = 0x06036ee0;
            }
            0x06036d0c => {
                r0 = 0x00000200; // 06036d0c mov.l 0x6036f58,r0
                r1 = 0i32 as u32; // 06036d0e mov #0,r1
                pc = 0x06036d10;
            }
            0x06036d10 => {
                t = r2 as i32 > r0 as i32; // 06036d10 cmp/gt r0,r2
                pc = if !t { 0x06036d1c } else { 0x06036d14 }; // 06036d12 bf 0x6036d1c
            }
            0x06036d14 => {
                r1 = 1i32 as u32; // 06036d14 mov #1,r1
                t = r2 & 1 != 0; r2 = (r2 as i32 >> 1) as u32; // 06036d16 shar r2
                // 06036d18 bra 0x6036d10
                t = r5 & 1 != 0; r5 = (r5 as i32 >> 1) as u32; // 06036d1a shar r5
                pc = 0x06036d10;
            }
            0x06036d1c => {
                t = r5 as i32 > r0 as i32; // 06036d1c cmp/gt r0,r5
                pc = if !t { 0x06036d28 } else { 0x06036d20 }; // 06036d1e bf 0x6036d28
            }
            0x06036d20 => {
                r1 = 1i32 as u32; // 06036d20 mov #1,r1
                t = r2 & 1 != 0; r2 = (r2 as i32 >> 1) as u32; // 06036d22 shar r2
                // 06036d24 bra 0x6036d1c
                t = r5 & 1 != 0; r5 = (r5 as i32 >> 1) as u32; // 06036d26 shar r5
                pc = 0x06036d1c;
            }
            0x06036d28 => {
                r0 = r0.wrapping_neg(); // 06036d28 neg r0,r0
                t = r2 as i32 >= r0 as i32; // 06036d2a cmp/ge r0,r2
                pc = if t { 0x06036d36 } else { 0x06036d2e }; // 06036d2c bt 0x6036d36
            }
            0x06036d2e => {
                r1 = 1i32 as u32; // 06036d2e mov #1,r1
                t = r2 & 1 != 0; r2 = (r2 as i32 >> 1) as u32; // 06036d30 shar r2
                // 06036d32 bra 0x6036d28
                t = r5 & 1 != 0; r5 = (r5 as i32 >> 1) as u32; // 06036d34 shar r5
                pc = 0x06036d28;
            }
            0x06036d36 => {
                t = r5 as i32 >= r0 as i32; // 06036d36 cmp/ge r0,r5
                pc = if t { 0x06036d42 } else { 0x06036d3a }; // 06036d38 bt 0x6036d42
            }
            0x06036d3a => {
                r1 = 1i32 as u32; // 06036d3a mov #1,r1
                t = r2 & 1 != 0; r2 = (r2 as i32 >> 1) as u32; // 06036d3c shar r2
                // 06036d3e bra 0x6036d36
                t = r5 & 1 != 0; r5 = (r5 as i32 >> 1) as u32; // 06036d40 shar r5
                pc = 0x06036d36;
            }
            0x06036d42 => {
                t = r1 & r1 == 0; // 06036d42 tst r1,r1
                pc = if !t { 0x06036d4a } else { 0x06036d46 }; // 06036d44 bf 0x6036d4a
            }
            0x06036d46 => {
                // 06036d46 bra 0x6036f46
                // nop // 06036d48 nop 
                pc = 0x06036f46;
            }
            0x06036d4a => {
                r0 = r2; // 06036d4a mov r2,r0
                r1 = r5; // 06036d4c mov r5,r1
                r4 = r2; // 06036d4e mov r2,r4
                r7 = r5; // 06036d50 mov r5,r7
                t = r0 as i32 >= 0; // 06036d52 cmp/pz r0
                pc = if t { 0x06036d58 } else { 0x06036d56 }; // 06036d54 bt 0x6036d58
            }
            0x06036d56 => {
                r0 = r0.wrapping_neg(); // 06036d56 neg r0,r0
                pc = 0x06036d58;
            }
            0x06036d58 => {
                t = r1 as i32 >= 0; // 06036d58 cmp/pz r1
                pc = if t { 0x06036d5e } else { 0x06036d5c }; // 06036d5a bt 0x6036d5e
            }
            0x06036d5c => {
                r1 = r1.wrapping_neg(); // 06036d5c neg r1,r1
                pc = 0x06036d5e;
            }
            0x06036d5e => {
                t = r1 as i32 >= r0 as i32; // 06036d5e cmp/ge r0,r1
                let take = t; // 06036d60 bt/s 0x6036d9e
                r0 = 4i32 as u32; // 06036d62 mov #4,r0
                pc = if take { 0x06036d9e } else { 0x06036d64 };
            }
            0x06036d64 => {
                t = r2 as i32 >= 0; // 06036d64 cmp/pz r2
                pc = if !t { 0x06036d82 } else { 0x06036d68 }; // 06036d66 bf 0x6036d82
            }
            0x06036d68 => {
                r1 = 0x00000200; // 06036d68 mov.l 0x6036f58,r1
                pc = 0x06036d6a;
            }
            0x06036d6a => {
                r2 = r2.wrapping_add(r4); // 06036d6a add r4,r2
                t = r2 as i32 > r1 as i32; // 06036d6c cmp/gt r1,r2
                let take = t; // 06036d6e bt/s 0x6036d76
                r2 = r2.wrapping_sub(r4); // 06036d70 sub r4,r2
                pc = if take { 0x06036d76 } else { 0x06036d72 };
            }
            0x06036d72 => {
                r5 = r5.wrapping_add(r7); // 06036d72 add r7,r5
                r2 = r2.wrapping_add(r4); // 06036d74 add r4,r2
                pc = 0x06036d76;
            }
            0x06036d76 => {
                t = r7 & 1 != 0; r7 = (r7 as i32 >> 1) as u32; // 06036d76 shar r7
                r0 = r0.wrapping_sub(1); t = r0 == 0; // 06036d78 dt r0
                let take = !t; // 06036d7a bf/s 0x6036d6a
                t = r4 & 1 != 0; r4 = (r4 as i32 >> 1) as u32; // 06036d7c shar r4
                pc = if take { 0x06036d6a } else { 0x06036d7e };
            }
            0x06036d7e => {
                // 06036d7e bra 0x6036f46
                // nop // 06036d80 nop 
                pc = 0x06036f46;
            }
            0x06036d82 => {
                r1 = 0x00000200; // 06036d82 mov.l 0x6036f58,r1
                r1 = r1.wrapping_neg(); // 06036d84 neg r1,r1
                pc = 0x06036d86;
            }
            0x06036d86 => {
                r2 = r2.wrapping_add(r4); // 06036d86 add r4,r2
                t = r2 as i32 >= r1 as i32; // 06036d88 cmp/ge r1,r2
                let take = !t; // 06036d8a bf/s 0x6036d92
                r2 = r2.wrapping_sub(r4); // 06036d8c sub r4,r2
                pc = if take { 0x06036d92 } else { 0x06036d8e };
            }
            0x06036d8e => {
                r5 = r5.wrapping_add(r7); // 06036d8e add r7,r5
                r2 = r2.wrapping_add(r4); // 06036d90 add r4,r2
                pc = 0x06036d92;
            }
            0x06036d92 => {
                t = r7 & 1 != 0; r7 = (r7 as i32 >> 1) as u32; // 06036d92 shar r7
                r0 = r0.wrapping_sub(1); t = r0 == 0; // 06036d94 dt r0
                let take = !t; // 06036d96 bf/s 0x6036d86
                t = r4 & 1 != 0; r4 = (r4 as i32 >> 1) as u32; // 06036d98 shar r4
                pc = if take { 0x06036d86 } else { 0x06036d9a };
            }
            0x06036d9a => {
                // 06036d9a bra 0x6036f46
                // nop // 06036d9c nop 
                pc = 0x06036f46;
            }
            0x06036d9e => {
                t = r5 as i32 >= 0; // 06036d9e cmp/pz r5
                pc = if !t { 0x06036dbc } else { 0x06036da2 }; // 06036da0 bf 0x6036dbc
            }
            0x06036da2 => {
                r1 = 0x00000200; // 06036da2 mov.l 0x6036f58,r1
                pc = 0x06036da4;
            }
            0x06036da4 => {
                r5 = r5.wrapping_add(r7); // 06036da4 add r7,r5
                t = r5 as i32 > r1 as i32; // 06036da6 cmp/gt r1,r5
                let take = t; // 06036da8 bt/s 0x6036db0
                r5 = r5.wrapping_sub(r7); // 06036daa sub r7,r5
                pc = if take { 0x06036db0 } else { 0x06036dac };
            }
            0x06036dac => {
                r2 = r2.wrapping_add(r4); // 06036dac add r4,r2
                r5 = r5.wrapping_add(r7); // 06036dae add r7,r5
                pc = 0x06036db0;
            }
            0x06036db0 => {
                t = r4 & 1 != 0; r4 = (r4 as i32 >> 1) as u32; // 06036db0 shar r4
                r0 = r0.wrapping_sub(1); t = r0 == 0; // 06036db2 dt r0
                let take = !t; // 06036db4 bf/s 0x6036da4
                t = r7 & 1 != 0; r7 = (r7 as i32 >> 1) as u32; // 06036db6 shar r7
                pc = if take { 0x06036da4 } else { 0x06036db8 };
            }
            0x06036db8 => {
                // 06036db8 bra 0x6036f46
                // nop // 06036dba nop 
                pc = 0x06036f46;
            }
            0x06036dbc => {
                r1 = 0x00000200; // 06036dbc mov.l 0x6036f58,r1
                r1 = r1.wrapping_neg(); // 06036dbe neg r1,r1
                pc = 0x06036dc0;
            }
            0x06036dc0 => {
                r5 = r5.wrapping_add(r7); // 06036dc0 add r7,r5
                t = r5 as i32 >= r1 as i32; // 06036dc2 cmp/ge r1,r5
                let take = !t; // 06036dc4 bf/s 0x6036dcc
                r5 = r5.wrapping_sub(r7); // 06036dc6 sub r7,r5
                pc = if take { 0x06036dcc } else { 0x06036dc8 };
            }
            0x06036dc8 => {
                r2 = r2.wrapping_add(r4); // 06036dc8 add r4,r2
                r5 = r5.wrapping_add(r7); // 06036dca add r7,r5
                pc = 0x06036dcc;
            }
            0x06036dcc => {
                t = r4 & 1 != 0; r4 = (r4 as i32 >> 1) as u32; // 06036dcc shar r4
                r0 = r0.wrapping_sub(1); t = r0 == 0; // 06036dce dt r0
                let take = !t; // 06036dd0 bf/s 0x6036dc0
                t = r7 & 1 != 0; r7 = (r7 as i32 >> 1) as u32; // 06036dd2 shar r7
                pc = if take { 0x06036dc0 } else { 0x06036dd4 };
            }
            0x06036dd4 => {
                // 06036dd4 bra 0x6036f46
                // nop // 06036dd6 nop 
                pc = 0x06036f46;
            }
            0x06036dd8 => {
                r15 = r15.wrapping_sub(4); cpu.write32(r15, pr); // 06036dd8 sts.l pr,@-r15
                r9 = r0; // 06036dda mov r0,r9
                r0 = cpu.read32(w.wrapping_add(40)); // 06036ddc mov.l @(40,gbr),r0
                r2 = r9; // 06036dde mov r9,r2
                r2 <<= 2; // 06036de0 shll2 r2
                r11 = r2; // 06036de2 mov r2,r11
                r2 = r2.wrapping_add(r11); // 06036de4 add r11,r2
                r2 = r2.wrapping_add(r11); // 06036de6 add r11,r2
                r11 <<= 2; // 06036de8 shll2 r11
                r11 = r11.wrapping_add(r0); // 06036dea add r0,r11
                r1 = cpu.read32(r11.wrapping_add(12)); // 06036dec mov.l @(12,r11),r1
                r14 = -1i32 as u32; // 06036dee mov #-1,r14
                r0 = cpu.read32(w.wrapping_add(108)); // 06036df0 mov.l @(108,gbr),r0
                r14 <<= 8; // 06036df2 shll8 r14
                cpu.write32(r14.wrapping_add(0), r1); // 06036df4 mov.l r1,@(0,r14)
                r5 = r0.rotate_left(16); // 06036df6 swap.w r0,r5
                r5 = r5 as i16 as i32 as u32; // 06036df8 exts.w r5,r5
                cpu.write32(r14.wrapping_add(16), r5); // 06036dfa mov.l r5,@(16,r14)
                r7 = r0 as i16 as i32 as u32; // 06036dfc exts.w r0,r7
                cpu.write32(r14.wrapping_add(20), r7); // 06036dfe mov.l r7,@(20,r14)
                r8 = r0; // 06036e00 mov r0,r8
                r10 = cpu.read32(r4); r4 = r4.wrapping_add(4); // 06036e02 mov.l @r4+,r10
                r2 = r2.wrapping_add(-12i32 as u32); // 06036e04 add #-12,r2
                r15 = r15.wrapping_sub(4); cpu.write32(r15, r4); // 06036e06 mov.l r4,@-r15
                r10 = r10.wrapping_add(r2); // 06036e08 add r2,r10
                r4 = r4.wrapping_add(12i32 as u32); // 06036e0a add #12,r4
                r0 = 0x06036f5c; // 06036e0c mova 0x6036f5c,r0
                r6 = r0; // 06036e0e mov r0,r6
                r0 = cpu.read32(r4.wrapping_add(0)); // 06036e10 mov.l @(0,r4),r0
                r1 = cpu.read32(r4.wrapping_add(4)); // 06036e12 mov.l @(4,r4),r1
                r2 = cpu.read32(r4.wrapping_add(8)); // 06036e14 mov.l @(8,r4),r2
                cpu.write32(r6.wrapping_add(0), r0); // 06036e16 mov.l r0,@(0,r6)
                cpu.write32(r6.wrapping_add(4), r1); // 06036e18 mov.l r1,@(4,r6)
                cpu.write32(r6.wrapping_add(8), r2); // 06036e1a mov.l r2,@(8,r6)
                r0 = cpu.read32(r4.wrapping_add(16)); // 06036e1c mov.l @(16,r4),r0
                r1 = cpu.read32(r4.wrapping_add(20)); // 06036e1e mov.l @(20,r4),r1
                r2 = cpu.read32(r4.wrapping_add(24)); // 06036e20 mov.l @(24,r4),r2
                cpu.write32(r6.wrapping_add(16), r0); // 06036e22 mov.l r0,@(16,r6)
                cpu.write32(r6.wrapping_add(20), r1); // 06036e24 mov.l r1,@(20,r6)
                cpu.write32(r6.wrapping_add(24), r2); // 06036e26 mov.l r2,@(24,r6)
                r0 = cpu.read32(r4.wrapping_add(32)); // 06036e28 mov.l @(32,r4),r0
                r1 = cpu.read32(r4.wrapping_add(36)); // 06036e2a mov.l @(36,r4),r1
                r2 = cpu.read32(r4.wrapping_add(40)); // 06036e2c mov.l @(40,r4),r2
                cpu.write32(r6.wrapping_add(32), r0); // 06036e2e mov.l r0,@(32,r6)
                cpu.write32(r6.wrapping_add(36), r1); // 06036e30 mov.l r1,@(36,r6)
                cpu.write32(r6.wrapping_add(40), r2); // 06036e32 mov.l r2,@(40,r6)
                r0 = cpu.read32(r4.wrapping_add(12)); // 06036e34 mov.l @(12,r4),r0
                r1 = cpu.read32(r4.wrapping_add(28)); // 06036e36 mov.l @(28,r4),r1
                r2 = cpu.read32(r4.wrapping_add(44)); // 06036e38 mov.l @(44,r4),r2
                cpu.write32(r6.wrapping_add(12), r0); // 06036e3a mov.l r0,@(12,r6)
                cpu.write32(r6.wrapping_add(28), r1); // 06036e3c mov.l r1,@(28,r6)
                cpu.write32(r6.wrapping_add(44), r2); // 06036e3e mov.l r2,@(44,r6)
                t = r9 & r9 == 0; // 06036e40 tst r9,r9
                let take = !t; // 06036e42 bf/s 0x6036e4a
                r6 = r6.wrapping_add(32i32 as u32); // 06036e44 add #32,r6
                pc = if take { 0x06036e4a } else { 0x06036e46 };
            }
            0x06036e46 => {
                // 06036e46 bra 0x6036f4c
                // nop // 06036e48 nop 
                pc = 0x06036f4c;
            }
            0x06036e4a => {
                r15 = r15.wrapping_sub(4); cpu.write32(r15, r5); // 06036e4a mov.l r5,@-r15
                r0 = r11; // 06036e4c mov r11,r0
                cpu.write32(w.wrapping_add(40), r0); // 06036e4e mov.l r0,@(40,gbr)
                r0 = cpu.read8(w.wrapping_add(173)) as i8 as i32 as u32; // 06036e50 mov.b @(173,gbr),r0
                r0 = r0.wrapping_add(1i32 as u32); // 06036e52 add #1,r0
                t = r8 & 1 != 0; r8 >>= 1; // 06036e54 shlr r8
                pc = 0x06036e56;
            }
            0x06036e56 => {
                r0 = r0.wrapping_sub(1); t = r0 == 0; // 06036e56 dt r0
                let take = !t; // 06036e58 bf/s 0x6036e56
                t = r8 & 1 != 0; r8 >>= 1; // 06036e5a shlr r8
                pc = if take { 0x06036e56 } else { 0x06036e5c };
            }
            0x06036e5c => {
                r0 = cpu.read32(w.wrapping_add(816)); // 06036e5c mov.l @(816,gbr),r0
                r15 = r15.wrapping_sub(4); cpu.write32(r15, r7); // 06036e5e mov.l r7,@-r15
                r13 = r0 & 0xffff; // 06036e60 extu.w r0,r13
                r4 = cpu.read32(r11.wrapping_add(4)); // 06036e62 mov.l @(4,r11),r4
                r12 = r0.rotate_left(16); // 06036e64 swap.w r0,r12
                r5 = cpu.read32(r11.wrapping_add(8)); // 06036e66 mov.l @(8,r11),r5
                r12 = r12 & 0xffff; // 06036e68 extu.w r12,r12
                r0 = cpu.read32(w.wrapping_add(908)); // 06036e6a mov.l @(908,gbr),r0
                r12 = r12.wrapping_add(r0); // 06036e6c add r0,r12
                r13 = r13.wrapping_add(r0); // 06036e6e add r0,r13
                r12 = r12.wrapping_add(r0); // 06036e70 add r0,r12
                r13 = r13.wrapping_add(r0); // 06036e72 add r0,r13
                r7 = cpu.read32(r14.wrapping_add(28)); // 06036e74 mov.l @(28,r14),r7
                pc = 0x06036e76;
            }
            0x06036e76 => {
                mac = 0; // 06036e76 clrmac 
                { let x = cpu.read32(r10) as i32 as i64; r10 = r10.wrapping_add(4); let y = cpu.read32(r6) as i32 as i64; r6 = r6.wrapping_add(4); mac = mac.wrapping_add(x * y); if cpu.sh2.sr & 2 != 0 { mac = mac.clamp(-0x8000_0000_0000, 0x7fff_ffff_ffff); } } // 06036e78 mac.l @r10+,@r6+
                r0 = cpu.read32(r15.wrapping_add(4)); // 06036e7a mov.l @(4,r15),r0
                { let x = cpu.read32(r10) as i32 as i64; r10 = r10.wrapping_add(4); let y = cpu.read32(r6) as i32 as i64; r6 = r6.wrapping_add(4); mac = mac.wrapping_add(x * y); if cpu.sh2.sr & 2 != 0 { mac = mac.clamp(-0x8000_0000_0000, 0x7fff_ffff_ffff); } } // 06036e7c mac.l @r10+,@r6+
                r1 = cpu.read32(r6.wrapping_add(4)); // 06036e7e mov.l @(4,r6),r1
                { let x = cpu.read32(r10) as i32 as i64; r10 = r10.wrapping_add(4); let y = cpu.read32(r6) as i32 as i64; r6 = r6.wrapping_add(4); mac = mac.wrapping_add(x * y); if cpu.sh2.sr & 2 != 0 { mac = mac.clamp(-0x8000_0000_0000, 0x7fff_ffff_ffff); } } // 06036e80 mac.l @r10+,@r6+
                r6 = r6.wrapping_add(-44i32 as u32); // 06036e82 add #-44,r6
                r10 = r10.wrapping_add(-12i32 as u32); // 06036e84 add #-12,r10
                r2 = (mac >> 32) as u32; // 06036e86 sts mach,r2
                r3 = mac as u32; // 06036e88 sts macl,r3
                cpu.write32(r14.wrapping_add(16), r0); // 06036e8a mov.l r0,@(16,r14)
                r3 = (r2 << 16) | (r3 >> 16); // 06036e8c xtrct r2,r3
                r3 = r3.wrapping_add(r1); // 06036e8e add r1,r3
                t = r3 as i32 >= r8 as i32; // 06036e90 cmp/ge r8,r3
                let take = t; // 06036e92 bt/s 0x6036e98
                r0 = cpu.read32(r15.wrapping_add(0)); // 06036e94 mov.l @(0,r15),r0
                pc = if take { 0x06036e98 } else { 0x06036e96 };
            }
            0x06036e96 => {
                r3 = r8; // 06036e96 mov r8,r3
                pc = 0x06036e98;
            }
            0x06036e98 => {
                cpu.write32(r14.wrapping_add(0), r3); // 06036e98 mov.l r3,@(0,r14)
                r11 = r11.wrapping_add(-16i32 as u32); // 06036e9a add #-16,r11
                cpu.write32(r14.wrapping_add(20), r0); // 06036e9c mov.l r0,@(20,r14)
                mac = r4 as i32 as i64 * r7 as i32 as i64; // 06036e9e dmuls.l r4,r7
                r0 = cpu.read16(w.wrapping_add(148)) as i16 as i32 as u32; // 06036ea0 mov.w @(148,gbr),r0
                t = r4 & 1 != 0; r4 >>= 1; // 06036ea2 shlr r4
                cpu.write32(r11.wrapping_add(12), r3); // 06036ea4 mov.l r3,@(12,r11)
                r3 = (mac >> 32) as u32; // 06036ea6 sts mach,r3
                mac = r5 as i32 as i64 * r7 as i32 as i64; // 06036ea8 dmuls.l r5,r7
                r0 = r0.wrapping_add(r3); // 06036eaa add r3,r0
                cpu.write32(r11.wrapping_add(24), r5); // 06036eac mov.l r5,@(24,r11)
                r1 = r0; // 06036eae mov r0,r1
                r0 = cpu.read32(w.wrapping_add(908)); // 06036eb0 mov.l @(908,gbr),r0
                r0 = r0.wrapping_add(r1); // 06036eb2 add r1,r0
                t = r0 >= r12; // 06036eb4 cmp/hs r12,r0
                { let (x, c1) = r4.overflowing_add(r4); let (y, c2) = x.overflowing_add(t as u32); r4 = y; t = c1 || c2; } // 06036eb6 addc r4,r4
                r5 = (mac >> 32) as u32; // 06036eb8 sts mach,r5
                r0 = cpu.read16(w.wrapping_add(150)) as i16 as i32 as u32; // 06036eba mov.w @(150,gbr),r0
                mac = 0; // 06036ebc clrmac 
                { let x = cpu.read32(r10) as i32 as i64; r10 = r10.wrapping_add(4); let y = cpu.read32(r6) as i32 as i64; r6 = r6.wrapping_add(4); mac = mac.wrapping_add(x * y); if cpu.sh2.sr & 2 != 0 { mac = mac.clamp(-0x8000_0000_0000, 0x7fff_ffff_ffff); } } // 06036ebe mac.l @r10+,@r6+
                r0 = r0.wrapping_add(r5); // 06036ec0 add r5,r0
                r1 = r0; // 06036ec2 mov r0,r1
                r0 = cpu.read32(w.wrapping_add(908)); // 06036ec4 mov.l @(908,gbr),r0
                r0 = r0.wrapping_add(r1); // 06036ec6 add r1,r0
                t = r0 >= r13; // 06036ec8 cmp/hs r13,r0
                { let x = cpu.read32(r10) as i32 as i64; r10 = r10.wrapping_add(4); let y = cpu.read32(r6) as i32 as i64; r6 = r6.wrapping_add(4); mac = mac.wrapping_add(x * y); if cpu.sh2.sr & 2 != 0 { mac = mac.clamp(-0x8000_0000_0000, 0x7fff_ffff_ffff); } } // 06036eca mac.l @r10+,@r6+
                r0 = t as u32; // 06036ecc movt r0
                r4 |= r0; // 06036ece or r0,r4
                { let x = cpu.read32(r10) as i32 as i64; r10 = r10.wrapping_add(4); let y = cpu.read32(r6) as i32 as i64; r6 = r6.wrapping_add(4); mac = mac.wrapping_add(x * y); if cpu.sh2.sr & 2 != 0 { mac = mac.clamp(-0x8000_0000_0000, 0x7fff_ffff_ffff); } } // 06036ed0 mac.l @r10+,@r6+
                cpu.write32(r11.wrapping_add(20), r4); // 06036ed2 mov.l r4,@(20,r11)
                r0 = cpu.read32(w.wrapping_add(892)); // 06036ed4 mov.l @(892,gbr),r0
                r4 &= r0; // 06036ed6 and r0,r4
                t = r4 & 1 != 0; r4 = (r4 as i32 >> 1) as u32; // 06036ed8 shar r4
                pc = if !t { 0x06036ee0 } else { 0x06036edc }; // 06036eda bf 0x6036ee0
            }
            0x06036edc => {
                // 06036edc bra 0x6036c40
                // nop // 06036ede nop 
                pc = 0x06036c40;
            }
            0x06036ee0 => {
                r5 <<= 16; // 06036ee0 shll16 r5
                r2 = cpu.read32(r6); r6 = r6.wrapping_add(4); // 06036ee2 mov.l @r6+,r2
                r10 = r10.wrapping_add(-12i32 as u32); // 06036ee4 add #-12,r10
                r1 = (mac >> 32) as u32; // 06036ee6 sts mach,r1
                r4 = mac as u32; // 06036ee8 sts macl,r4
                mac = 0; // 06036eea clrmac 
                { let x = cpu.read32(r10) as i32 as i64; r10 = r10.wrapping_add(4); let y = cpu.read32(r6) as i32 as i64; r6 = r6.wrapping_add(4); mac = mac.wrapping_add(x * y); if cpu.sh2.sr & 2 != 0 { mac = mac.clamp(-0x8000_0000_0000, 0x7fff_ffff_ffff); } } // 06036eec mac.l @r10+,@r6+
                r5 = (r3 << 16) | (r5 >> 16); // 06036eee xtrct r3,r5
                cpu.write32(r11.wrapping_add(16), r5); // 06036ef0 mov.l r5,@(16,r11)
                { let x = cpu.read32(r10) as i32 as i64; r10 = r10.wrapping_add(4); let y = cpu.read32(r6) as i32 as i64; r6 = r6.wrapping_add(4); mac = mac.wrapping_add(x * y); if cpu.sh2.sr & 2 != 0 { mac = mac.clamp(-0x8000_0000_0000, 0x7fff_ffff_ffff); } } // 06036ef2 mac.l @r10+,@r6+
                r4 = (r1 << 16) | (r4 >> 16); // 06036ef4 xtrct r1,r4
                r4 = r4.wrapping_add(r2); // 06036ef6 add r2,r4
                { let x = cpu.read32(r10) as i32 as i64; r10 = r10.wrapping_add(4); let y = cpu.read32(r6) as i32 as i64; r6 = r6.wrapping_add(4); mac = mac.wrapping_add(x * y); if cpu.sh2.sr & 2 != 0 { mac = mac.clamp(-0x8000_0000_0000, 0x7fff_ffff_ffff); } } // 06036ef8 mac.l @r10+,@r6+
                r2 = cpu.read32(r6); // 06036efa mov.l @r6,r2
                r6 = r6.wrapping_add(4i32 as u32); // 06036efc add #4,r6
                r10 = r10.wrapping_add(-24i32 as u32); // 06036efe add #-24,r10
                r1 = (mac >> 32) as u32; // 06036f00 sts mach,r1
                r5 = mac as u32; // 06036f02 sts macl,r5
                r7 = cpu.read32(r14.wrapping_add(28)); // 06036f04 mov.l @(28,r14),r7
                r5 = (r1 << 16) | (r5 >> 16); // 06036f06 xtrct r1,r5
                r9 = r9.wrapping_sub(1); t = r9 == 0; // 06036f08 dt r9
                let take = !t; // 06036f0a bf/s 0x6036e76
                r5 = r5.wrapping_add(r2); // 06036f0c add r2,r5
                pc = if take { 0x06036e76 } else { 0x06036f0e };
            }
            0x06036f0e => {
                mac = r4 as i32 as i64 * r7 as i32 as i64; // 06036f0e dmuls.l r4,r7
                r0 = cpu.read16(w.wrapping_add(148)) as i16 as i32 as u32; // 06036f10 mov.w @(148,gbr),r0
                t = r4 & 1 != 0; r4 >>= 1; // 06036f12 shlr r4
                cpu.write32(r11.wrapping_add(8), r5); // 06036f14 mov.l r5,@(8,r11)
                r2 = (mac >> 32) as u32; // 06036f16 sts mach,r2
                mac = r5 as i32 as i64 * r7 as i32 as i64; // 06036f18 dmuls.l r5,r7
                r0 = r0.wrapping_add(r2); // 06036f1a add r2,r0
                r1 = r0; // 06036f1c mov r0,r1
                r0 = cpu.read32(w.wrapping_add(908)); // 06036f1e mov.l @(908,gbr),r0
                r0 = r0.wrapping_add(r1); // 06036f20 add r1,r0
                t = r0 >= r12; // 06036f22 cmp/hs r12,r0
                r5 = (mac >> 32) as u32; // 06036f24 sts mach,r5
                r0 = cpu.read16(w.wrapping_add(150)) as i16 as i32 as u32; // 06036f26 mov.w @(150,gbr),r0
                { let (x, c1) = r4.overflowing_add(r4); let (y, c2) = x.overflowing_add(t as u32); r4 = y; t = c1 || c2; } // 06036f28 addc r4,r4
                r0 = r0.wrapping_add(r5); // 06036f2a add r5,r0
                r1 = r0; // 06036f2c mov r0,r1
                r0 = cpu.read32(w.wrapping_add(908)); // 06036f2e mov.l @(908,gbr),r0
                r0 = r0.wrapping_add(r1); // 06036f30 add r1,r0
                t = r0 >= r13; // 06036f32 cmp/hs r13,r0
                r0 = t as u32; // 06036f34 movt r0
                r4 |= r0; // 06036f36 or r0,r4
                cpu.write32(r11.wrapping_add(4), r4); // 06036f38 mov.l r4,@(4,r11)
                r0 = cpu.read32(w.wrapping_add(892)); // 06036f3a mov.l @(892,gbr),r0
                r4 &= r0; // 06036f3c and r0,r4
                t = r4 & 1 != 0; r4 = (r4 as i32 >> 1) as u32; // 06036f3e shar r4
                pc = if !t { 0x06036f46 } else { 0x06036f42 }; // 06036f40 bf 0x6036f46
            }
            0x06036f42 => {
                // 06036f42 bra 0x6036d0c
                // nop // 06036f44 nop 
                pc = 0x06036d0c;
            }
            0x06036f46 => {
                r5 <<= 16; // 06036f46 shll16 r5
                r5 = (r2 << 16) | (r5 >> 16); // 06036f48 xtrct r2,r5
                cpu.write32(r11.wrapping_add(0), r5); // 06036f4a mov.l r5,@(0,r11)
                pc = 0x06036f4c;
            }
            0x06036f4c => {
                r10 = r11; // 06036f4c mov r11,r10
                r15 = r15.wrapping_add(8i32 as u32); // 06036f4e add #8,r15
                r4 = cpu.read32(r15); r15 = r15.wrapping_add(4); // 06036f50 mov.l @r15+,r4
                // 06036f52 bra 0x6036f90
                r6 = r6.wrapping_add(-32i32 as u32); // 06036f54 add #-32,r6
                pc = 0x06036f90;
            }
            0x06036f90 => {
                r0 = cpu.read8(w.wrapping_add(169)) as i8 as i32 as u32; // 06036f90 mov.b @(169,gbr),r0
                t = r0 & 1 == 0; // 06036f92 tst #1,r0
                let take = !t; // 06036f94 bf/s 0x6036f9a
                r1 = 32i32 as u32; // 06036f96 mov #32,r1
                pc = if take { 0x06036f9a } else { 0x06036f98 };
            }
            0x06036f98 => {
                r1 = 16i32 as u32; // 06036f98 mov #16,r1
                pc = 0x06036f9a;
            }
            0x06036f9a => {
                r0 |= r1; // 06036f9a or r1,r0
                cpu.write8(w.wrapping_add(169), r0 as u8); // 06036f9c mov.b r0,@(169,gbr)
                r0 = cpu.read32(w.wrapping_add(52)); // 06036f9e mov.l @(52,gbr),r0
                r3 = cpu.read32(r4); r4 = r4.wrapping_add(4); // 06036fa0 mov.l @r4+,r3
                r7 = cpu.read32(r4); r4 = r4.wrapping_add(4); // 06036fa2 mov.l @r4+,r7
                r8 = cpu.read32(r4); r4 = r4.wrapping_add(4); // 06036fa4 mov.l @r4+,r8
                r11 = r0; // 06036fa6 mov r0,r11
                r0 = cpu.read32(w.wrapping_add(60)); // 06036fa8 mov.l @(60,gbr),r0
                r15 = r15.wrapping_sub(4); cpu.write32(r15, r4); // 06036faa mov.l r4,@-r15
                r0 = r0.wrapping_add(4i32 as u32); // 06036fac add #4,r0
                r15 = r15.wrapping_sub(4); cpu.write32(r15, r0); // 06036fae mov.l r0,@-r15
                r15 = r15.wrapping_sub(4); cpu.write32(r15, r3); // 06036fb0 mov.l r3,@-r15
                r0 = cpu.read32(w.wrapping_add(108)); // 06036fb2 mov.l @(108,gbr),r0
                r14 = -1i32 as u32; // 06036fb4 mov #-1,r14
                r1 = r0; // 06036fb6 mov r0,r1
                r0 = cpu.read8(w.wrapping_add(173)) as i8 as i32 as u32; // 06036fb8 mov.b @(173,gbr),r0
                r1 >>= 16; // 06036fba shlr16 r1
                pc = 0x06036fbc;
            }
            0x06036fbc => {
                r0 = r0.wrapping_sub(1); t = r0 == 0; // 06036fbc dt r0
                let take = !t; // 06036fbe bf/s 0x6036fbc
                t = r1 & 1 != 0; r1 >>= 1; // 06036fc0 shlr r1
                pc = if take { 0x06036fbc } else { 0x06036fc2 };
            }
            0x06036fc2 => {
                r15 = r15.wrapping_sub(4); cpu.write32(r15, r1); // 06036fc2 mov.l r1,@-r15
                r15 = r15.wrapping_add(-12i32 as u32); // 06036fc4 add #-12,r15
                r0 = 0x06037074; // 06036fc6 mova 0x6037074,r0
                pr = r0; // 06036fc8 lds r0,pr
                pc = 0x06036fca;
            }
            0x06036fca => {
                mac = 0; // 06036fca clrmac 
                { let x = cpu.read32(r7) as i32 as i64; r7 = r7.wrapping_add(4); let y = cpu.read32(r6) as i32 as i64; r6 = r6.wrapping_add(4); mac = mac.wrapping_add(x * y); if cpu.sh2.sr & 2 != 0 { mac = mac.clamp(-0x8000_0000_0000, 0x7fff_ffff_ffff); } } // 06036fcc mac.l @r7+,@r6+
                r0 = cpu.read16(r7.wrapping_add(8)) as i16 as i32 as u32; // 06036fce mov.w @(8,r7),r0
                r5 = r15; // 06036fd0 mov r15,r5
                { let x = cpu.read32(r7) as i32 as i64; r7 = r7.wrapping_add(4); let y = cpu.read32(r6) as i32 as i64; r6 = r6.wrapping_add(4); mac = mac.wrapping_add(x * y); if cpu.sh2.sr & 2 != 0 { mac = mac.clamp(-0x8000_0000_0000, 0x7fff_ffff_ffff); } } // 06036fd2 mac.l @r7+,@r6+
                r1 = r0 as i16 as i32 as u32; // 06036fd4 exts.w r0,r1
                r1 <<= 2; // 06036fd6 shll2 r1
                { let x = cpu.read32(r7) as i32 as i64; r7 = r7.wrapping_add(4); let y = cpu.read32(r6) as i32 as i64; r6 = r6.wrapping_add(4); mac = mac.wrapping_add(x * y); if cpu.sh2.sr & 2 != 0 { mac = mac.clamp(-0x8000_0000_0000, 0x7fff_ffff_ffff); } } // 06036fd8 mac.l @r7+,@r6+
                r6 = r6.wrapping_add(4i32 as u32); // 06036fda add #4,r6
                r7 = r7.wrapping_add(-12i32 as u32); // 06036fdc add #-12,r7
                r1 <<= 2; // 06036fde shll2 r1
                r0 = (mac >> 32) as u32; // 06036fe0 sts mach,r0
                r13 = mac as u32; // 06036fe2 sts macl,r13
                mac = 0; // 06036fe4 clrmac 
                { let x = cpu.read32(r7) as i32 as i64; r7 = r7.wrapping_add(4); let y = cpu.read32(r6) as i32 as i64; r6 = r6.wrapping_add(4); mac = mac.wrapping_add(x * y); if cpu.sh2.sr & 2 != 0 { mac = mac.clamp(-0x8000_0000_0000, 0x7fff_ffff_ffff); } } // 06036fe6 mac.l @r7+,@r6+
                r13 = (r0 << 16) | (r13 >> 16); // 06036fe8 xtrct r0,r13
                cpu.write32(r5.wrapping_add(0), r13); // 06036fea mov.l r13,@(0,r5)
                { let x = cpu.read32(r7) as i32 as i64; r7 = r7.wrapping_add(4); let y = cpu.read32(r6) as i32 as i64; r6 = r6.wrapping_add(4); mac = mac.wrapping_add(x * y); if cpu.sh2.sr & 2 != 0 { mac = mac.clamp(-0x8000_0000_0000, 0x7fff_ffff_ffff); } } // 06036fec mac.l @r7+,@r6+
                r1 = r1.wrapping_add(r10); // 06036fee add r10,r1
                r4 = 4i32 as u32; // 06036ff0 mov #4,r4
                { let x = cpu.read32(r7) as i32 as i64; r7 = r7.wrapping_add(4); let y = cpu.read32(r6) as i32 as i64; r6 = r6.wrapping_add(4); mac = mac.wrapping_add(x * y); if cpu.sh2.sr & 2 != 0 { mac = mac.clamp(-0x8000_0000_0000, 0x7fff_ffff_ffff); } } // 06036ff2 mac.l @r7+,@r6+
                r6 = r6.wrapping_add(4i32 as u32); // 06036ff4 add #4,r6
                r7 = r7.wrapping_add(-12i32 as u32); // 06036ff6 add #-12,r7
                r4 = r4.wrapping_add(r1); // 06036ff8 add r1,r4
                r0 = (mac >> 32) as u32; // 06036ffa sts mach,r0
                r2 = mac as u32; // 06036ffc sts macl,r2
                mac = 0; // 06036ffe clrmac 
                { let x = cpu.read32(r7) as i32 as i64; r7 = r7.wrapping_add(4); let y = cpu.read32(r6) as i32 as i64; r6 = r6.wrapping_add(4); mac = mac.wrapping_add(x * y); if cpu.sh2.sr & 2 != 0 { mac = mac.clamp(-0x8000_0000_0000, 0x7fff_ffff_ffff); } } // 06037000 mac.l @r7+,@r6+
                r2 = (r0 << 16) | (r2 >> 16); // 06037002 xtrct r0,r2
                cpu.write32(r5.wrapping_add(4), r2); // 06037004 mov.l r2,@(4,r5)
                { let x = cpu.read32(r7) as i32 as i64; r7 = r7.wrapping_add(4); let y = cpu.read32(r6) as i32 as i64; r6 = r6.wrapping_add(4); mac = mac.wrapping_add(x * y); if cpu.sh2.sr & 2 != 0 { mac = mac.clamp(-0x8000_0000_0000, 0x7fff_ffff_ffff); } } // 06037006 mac.l @r7+,@r6+
                r12 = cpu.read16(r8) as i16 as i32 as u32; // 06037008 mov.w @r8,r12
                r9 = 127i32 as u32; // 0603700a mov #127,r9
                { let x = cpu.read32(r7) as i32 as i64; r7 = r7.wrapping_add(4); let y = cpu.read32(r6) as i32 as i64; r6 = r6.wrapping_add(4); mac = mac.wrapping_add(x * y); if cpu.sh2.sr & 2 != 0 { mac = mac.clamp(-0x8000_0000_0000, 0x7fff_ffff_ffff); } } // 0603700c mac.l @r7+,@r6+
                r6 = r6.wrapping_add(-44i32 as u32); // 0603700e add #-44,r6
                r9 &= r12; // 06037010 and r12,r9
                r9 <<= 2; // 06037012 shll2 r9
                r0 = (mac >> 32) as u32; // 06037014 sts mach,r0
                r3 = mac as u32; // 06037016 sts macl,r3
                mac = 0; // 06037018 clrmac 
                { let x = cpu.read32(r4) as i32 as i64; r4 = r4.wrapping_add(4); let y = cpu.read32(r5) as i32 as i64; r5 = r5.wrapping_add(4); mac = mac.wrapping_add(x * y); if cpu.sh2.sr & 2 != 0 { mac = mac.clamp(-0x8000_0000_0000, 0x7fff_ffff_ffff); } } // 0603701a mac.l @r4+,@r5+
                r3 = (r0 << 16) | (r3 >> 16); // 0603701c xtrct r0,r3
                r0 = 0x06037318; // 0603701e mova 0x6037318,r0
                { let x = cpu.read32(r4) as i32 as i64; r4 = r4.wrapping_add(4); let y = cpu.read32(r5) as i32 as i64; r5 = r5.wrapping_add(4); mac = mac.wrapping_add(x * y); if cpu.sh2.sr & 2 != 0 { mac = mac.clamp(-0x8000_0000_0000, 0x7fff_ffff_ffff); } } // 06037020 mac.l @r4+,@r5+
                cpu.write32(r5, r3); // 06037022 mov.l r3,@r5
                r9 <<= 2; // 06037024 shll2 r9
                { let x = cpu.read32(r4) as i32 as i64; r4 = r4.wrapping_add(4); let y = cpu.read32(r5) as i32 as i64; r5 = r5.wrapping_add(4); mac = mac.wrapping_add(x * y); if cpu.sh2.sr & 2 != 0 { mac = mac.clamp(-0x8000_0000_0000, 0x7fff_ffff_ffff); } } // 06037026 mac.l @r4+,@r5+
                r9 = r9.wrapping_add(r0); // 06037028 add r0,r9
                r0 = cpu.read16(r8.wrapping_add(10)) as i16 as i32 as u32; // 0603702a mov.w @(10,r8),r0
                r7 = r7.wrapping_add(2i32 as u32); // 0603702c add #2,r7
                r4 = (mac >> 32) as u32; // 0603702e sts mach,r4
                cpu.write16(r11.wrapping_add(0), r0 as u16); // 06037030 mov.w r0,@(0,r11)
                r0 = cpu.read32(w.wrapping_add(900)); // 06037032 mov.l @(900,gbr),r0
                t = r4 as i32 >= r0 as i32; // 06037034 cmp/ge r0,r4
                let take = t; // 06037036 bt/s 0x6037042
                r0 = (r12 & 0xffff_0000) | ((r12 & 0xff) << 8) | ((r12 >> 8) & 0xff); // 06037038 swap.b r12,r0
                pc = if take { 0x06037042 } else { 0x0603703a };
            }
            0x0603703a => {
                r0 = cpu.read32(r9); r9 = r9.wrapping_add(4); // 0603703a mov.l @r9+,r0
                r2 = cpu.read16(r7) as i16 as i32 as u32; r7 = r7.wrapping_add(2); // 0603703c mov.w @r7+,r2
                let to = r0; // 0603703e jmp @r0
                r2 <<= 2; // 06037040 shll2 r2
                pc = to;
            }
            0x06037042 => {
                t = r0 & 1 == 0; // 06037042 tst #1,r0
                let take = t; // 06037044 bt/s 0x603705a
                r3 = r3.wrapping_neg(); // 06037046 neg r3,r3
                pc = if take { 0x0603705a } else { 0x06037048 };
            }
            0x06037048 => {
                r5 = r5.wrapping_sub(4); cpu.write32(r5, r3); // 06037048 mov.l r3,@-r5
                r2 = r2.wrapping_neg(); // 0603704a neg r2,r2
                r5 = r5.wrapping_sub(4); cpu.write32(r5, r2); // 0603704c mov.l r2,@-r5
                r13 = r13.wrapping_neg(); // 0603704e neg r13,r13
                r5 = r5.wrapping_sub(4); cpu.write32(r5, r13); // 06037050 mov.l r13,@-r5
                r0 = cpu.read32(r9); r9 = r9.wrapping_add(4); // 06037052 mov.l @r9+,r0
                r2 = cpu.read16(r7) as i16 as i32 as u32; r7 = r7.wrapping_add(2); // 06037054 mov.w @r7+,r2
                let to = r0; // 06037056 jmp @r0
                r2 <<= 2; // 06037058 shll2 r2
                pc = to;
            }
            0x0603705a => {
                // 0603705a bra 0x6037088
                r7 = r7.wrapping_add(6i32 as u32); // 0603705c add #6,r7
                pc = 0x06037088;
            }
            0x06037060 => {
                r0 = cpu.read16(r8.wrapping_add(2)) as i16 as i32 as u32; // 06037060 mov.w @(2,r8),r0
                r5 = cpu.read32(r15.wrapping_add(20)); // 06037062 mov.l @(20,r15),r5
                r0 <<= 2; // 06037064 shll2 r0
                r0 = r0.wrapping_add(r0); // 06037066 add r0,r0
                r0 = cpu.read32(r0.wrapping_add(r5)); // 06037068 mov.l @(r0,r5),r0
                cpu.write32(r11.wrapping_add(8), r0); // 0603706a mov.l r0,@(8,r11)
                pc = 0x0603706c;
            }
            0x0603706c => {
                r0 = cpu.read16(r8.wrapping_add(8)) as i16 as i32 as u32; // 0603706c mov.w @(8,r8),r0
                r5 = cpu.read32(r8.wrapping_add(4)); // 0603706e mov.l @(4,r8),r5
                cpu.write16(r11.wrapping_add(28), r0 as u16); // 06037070 mov.w r0,@(28,r11)
                cpu.write32(r11.wrapping_add(4), r5); // 06037072 mov.l r5,@(4,r11)
                pc = 0x06037074;
            }
            0x06037074 => {
                r0 = r14; // 06037074 mov r14,r0
                cpu.write16(r11.wrapping_add(30), r0 as u16); // 06037076 mov.w r0,@(30,r11)
                r0 = cpu.read32(w.wrapping_add(56)); // 06037078 mov.l @(56,gbr),r0
                r1 = (r14 & 0xffff_0000) | ((r14 & 0xff) << 8) | ((r14 >> 8) & 0xff); // 0603707a swap.b r14,r1
                r1 = r1 & 0xff; // 0603707c extu.b r1,r1
                r1 <<= 2; // 0603707e shll2 r1
                r2 = cpu.read32(r0.wrapping_add(r1)); // 06037080 mov.l @(r0,r1),r2
                cpu.write32(r0.wrapping_add(r1), r11); // 06037082 mov.l r11,@(r0,r1)
                cpu.write32(r11.wrapping_add(32), r2); // 06037084 mov.l r2,@(32,r11)
                r11 = r11.wrapping_add(36i32 as u32); // 06037086 add #36,r11
                pc = 0x06037088;
            }
            0x06037088 => {
                r0 = cpu.read32(r15.wrapping_add(16)); // 06037088 mov.l @(16,r15),r0
                r8 = r8.wrapping_add(12i32 as u32); // 0603708a add #12,r8
                r0 = r0.wrapping_sub(1); t = r0 == 0; // 0603708c dt r0
                let take = !t; // 0603708e bf/s 0x6036fca
                cpu.write32(r15.wrapping_add(16), r0); // 06037090 mov.l r0,@(16,r15)
                pc = if take { 0x06036fca } else { 0x06037092 };
            }
            0x06037092 => {
                r15 = r15.wrapping_add(24i32 as u32); // 06037092 add #24,r15
                r4 = cpu.read32(r15); r15 = r15.wrapping_add(4); // 06037094 mov.l @r15+,r4
                pr = cpu.read32(r15); r15 = r15.wrapping_add(4); // 06037096 lds.l @r15+,pr
                r4 = r4.wrapping_add(48i32 as u32); // 06037098 add #48,r4
                r0 = r11; // 0603709a mov r11,r0
                // 0603709c rts
                cpu.write32(w.wrapping_add(52), r0); // 0603709e mov.l r0,@(52,gbr)
                if pr == entry_pr { break; }
                pc = pr;
            }
            0x060370a0 => {
                r3 = cpu.read16(r7) as i16 as i32 as u32; r7 = r7.wrapping_add(2); // 060370a0 mov.w @r7+,r3
                r2 <<= 2; // 060370a2 shll2 r2
                r14 = cpu.read32(r1.wrapping_add(12)); // 060370a4 mov.l @(12,r1),r14
                r2 = r2.wrapping_add(r10); // 060370a6 add r10,r2
                r4 = cpu.read16(r7) as i16 as i32 as u32; r7 = r7.wrapping_add(2); // 060370a8 mov.w @r7+,r4
                r0 = cpu.read32(r2.wrapping_add(12)); // 060370aa mov.l @(12,r2),r0
                r3 <<= 2; // 060370ac shll2 r3
                r14 = r14.wrapping_add(r0); // 060370ae add r0,r14
                r3 <<= 2; // 060370b0 shll2 r3
                r3 = r3.wrapping_add(r10); // 060370b2 add r10,r3
                r4 <<= 2; // 060370b4 shll2 r4
                r5 = cpu.read32(r3.wrapping_add(12)); // 060370b6 mov.l @(12,r3),r5
                r4 <<= 2; // 060370b8 shll2 r4
                r4 = r4.wrapping_add(r10); // 060370ba add r10,r4
                r0 = cpu.read32(r4.wrapping_add(12)); // 060370bc mov.l @(12,r4),r0
                r5 = r5.wrapping_add(r0); // 060370be add r0,r5
                t = r14 & 1 != 0; r14 = (r14 as i32 >> 1) as u32; // 060370c0 shar r14
                t = r5 & 1 != 0; r5 = (r5 as i32 >> 1) as u32; // 060370c2 shar r5
                r14 = r14.wrapping_add(r5); // 060370c4 add r5,r14
                t = r14 & 1 != 0; r14 = (r14 as i32 >> 1) as u32; // 060370c6 shar r14
                r14 >>= 16; // 060370c8 shlr16 r14
                r14 = r14 as i16 as i32 as u32; // 060370ca exts.w r14,r14
                r5 = cpu.read32(r15.wrapping_add(12)); // 060370cc mov.l @(12,r15),r5
                r0 = cpu.read16(w.wrapping_add(160)) as i16 as i32 as u32; // 060370ce mov.w @(160,gbr),r0
                r14 = r14.wrapping_sub(r5); // 060370d0 sub r5,r14
                t = r14 > r0; // 060370d2 cmp/hi r0,r14
                let take = t; // 060370d4 bt/s 0x6037088
                r14 = r14.wrapping_add(r5); // 060370d6 add r5,r14
                pc = if take { 0x06037088 } else { 0x060370d8 };
            }
            0x060370d8 => {
                r0 = cpu.read32(r9); r9 = r9.wrapping_add(4); // 060370d8 mov.l @r9+,r0
                let to = r0; // 060370da jmp @r0
                // nop // 060370dc nop 
                pc = to;
            }
            0x060370e0 => {
                r0 = cpu.read16(r1.wrapping_add(12)) as i16 as i32 as u32; // 060370e0 mov.w @(12,r1),r0
                r2 <<= 2; // 060370e2 shll2 r2
                r3 = cpu.read16(r7) as i16 as i32 as u32; r7 = r7.wrapping_add(2); // 060370e4 mov.w @r7+,r3
                r14 = r0 as i16 as i32 as u32; // 060370e6 exts.w r0,r14
                r4 = cpu.read16(r7) as i16 as i32 as u32; r7 = r7.wrapping_add(2); // 060370e8 mov.w @r7+,r4
                r2 = r2.wrapping_add(r10); // 060370ea add r10,r2
                r0 = cpu.read16(r2.wrapping_add(12)) as i16 as i32 as u32; // 060370ec mov.w @(12,r2),r0
                r3 <<= 2; // 060370ee shll2 r3
                t = r14 as i32 > r0 as i32; // 060370f0 cmp/gt r0,r14
                let take = !t; // 060370f2 bf/s 0x60370f8
                r3 <<= 2; // 060370f4 shll2 r3
                pc = if take { 0x060370f8 } else { 0x060370f6 };
            }
            0x060370f6 => {
                r14 = r0; // 060370f6 mov r0,r14
                pc = 0x060370f8;
            }
            0x060370f8 => {
                r3 = r3.wrapping_add(r10); // 060370f8 add r10,r3
                r4 <<= 2; // 060370fa shll2 r4
                r0 = cpu.read16(r3.wrapping_add(12)) as i16 as i32 as u32; // 060370fc mov.w @(12,r3),r0
                r4 <<= 2; // 060370fe shll2 r4
                t = r14 as i32 > r0 as i32; // 06037100 cmp/gt r0,r14
                let take = !t; // 06037102 bf/s 0x6037108
                r4 = r4.wrapping_add(r10); // 06037104 add r10,r4
                pc = if take { 0x06037108 } else { 0x06037106 };
            }
            0x06037106 => {
                r14 = r0; // 06037106 mov r0,r14
                pc = 0x06037108;
            }
            0x06037108 => {
                r0 = cpu.read16(r4.wrapping_add(12)) as i16 as i32 as u32; // 06037108 mov.w @(12,r4),r0
                r5 = cpu.read32(r15.wrapping_add(12)); // 0603710a mov.l @(12,r15),r5
                t = r14 as i32 > r0 as i32; // 0603710c cmp/gt r0,r14
                pc = if !t { 0x06037112 } else { 0x06037110 }; // 0603710e bf 0x6037112
            }
            0x06037110 => {
                r14 = r0; // 06037110 mov r0,r14
                pc = 0x06037112;
            }
            0x06037112 => {
                r0 = cpu.read16(w.wrapping_add(160)) as i16 as i32 as u32; // 06037112 mov.w @(160,gbr),r0
                r14 = r14.wrapping_sub(r5); // 06037114 sub r5,r14
                t = r14 > r0; // 06037116 cmp/hi r0,r14
                let take = t; // 06037118 bt/s 0x6037088
                r14 = r14.wrapping_add(r5); // 0603711a add r5,r14
                pc = if take { 0x06037088 } else { 0x0603711c };
            }
            0x0603711c => {
                r0 = cpu.read32(r9); r9 = r9.wrapping_add(4); // 0603711c mov.l @r9+,r0
                let to = r0; // 0603711e jmp @r0
                // nop // 06037120 nop 
                pc = to;
            }
            0x06037124 => {
                r3 = cpu.read16(r7) as i16 as i32 as u32; r7 = r7.wrapping_add(2); // 06037124 mov.w @r7+,r3
                r2 <<= 2; // 06037126 shll2 r2
                r0 = cpu.read16(r1.wrapping_add(12)) as i16 as i32 as u32; // 06037128 mov.w @(12,r1),r0
                r2 = r2.wrapping_add(r10); // 0603712a add r10,r2
                r4 = cpu.read16(r7) as i16 as i32 as u32; r7 = r7.wrapping_add(2); // 0603712c mov.w @r7+,r4
                r14 = r0 as i16 as i32 as u32; // 0603712e exts.w r0,r14
                r0 = cpu.read16(r2.wrapping_add(12)) as i16 as i32 as u32; // 06037130 mov.w @(12,r2),r0
                r3 <<= 2; // 06037132 shll2 r3
                r3 <<= 2; // 06037134 shll2 r3
                r3 = r3.wrapping_add(r10); // 06037136 add r10,r3
                t = r14 as i32 >= r0 as i32; // 06037138 cmp/ge r0,r14
                let take = t; // 0603713a bt/s 0x6037140
                r4 <<= 2; // 0603713c shll2 r4
                pc = if take { 0x06037140 } else { 0x0603713e };
            }
            0x0603713e => {
                r14 = r0; // 0603713e mov r0,r14
                pc = 0x06037140;
            }
            0x06037140 => {
                r0 = cpu.read16(r3.wrapping_add(12)) as i16 as i32 as u32; // 06037140 mov.w @(12,r3),r0
                r4 <<= 2; // 06037142 shll2 r4
                t = r14 as i32 >= r0 as i32; // 06037144 cmp/ge r0,r14
                let take = t; // 06037146 bt/s 0x603714c
                r4 = r4.wrapping_add(r10); // 06037148 add r10,r4
                pc = if take { 0x0603714c } else { 0x0603714a };
            }
            0x0603714a => {
                r14 = r0; // 0603714a mov r0,r14
                pc = 0x0603714c;
            }
            0x0603714c => {
                r0 = cpu.read16(r4.wrapping_add(12)) as i16 as i32 as u32; // 0603714c mov.w @(12,r4),r0
                r5 = cpu.read32(r15.wrapping_add(12)); // 0603714e mov.l @(12,r15),r5
                t = r14 as i32 >= r0 as i32; // 06037150 cmp/ge r0,r14
                pc = if t { 0x06037156 } else { 0x06037154 }; // 06037152 bt 0x6037156
            }
            0x06037154 => {
                r14 = r0; // 06037154 mov r0,r14
                pc = 0x06037156;
            }
            0x06037156 => {
                r0 = cpu.read16(w.wrapping_add(160)) as i16 as i32 as u32; // 06037156 mov.w @(160,gbr),r0
                r14 = r14.wrapping_sub(r5); // 06037158 sub r5,r14
                t = r14 > r0; // 0603715a cmp/hi r0,r14
                let take = t; // 0603715c bt/s 0x6037088
                r14 = r14.wrapping_add(r5); // 0603715e add r5,r14
                pc = if take { 0x06037088 } else { 0x06037160 };
            }
            0x06037160 => {
                r0 = cpu.read32(r9); r9 = r9.wrapping_add(4); // 06037160 mov.l @r9+,r0
                let to = r0; // 06037162 jmp @r0
                // nop // 06037164 nop 
                pc = to;
            }
            0x06037168 => {
                r0 = cpu.read16(w.wrapping_add(160)) as i16 as i32 as u32; // 06037168 mov.w @(160,gbr),r0
                r2 <<= 2; // 0603716a shll2 r2
                t = r14 > r0; // 0603716c cmp/hi r0,r14
                let take = t; // 0603716e bt/s 0x6037186
                r2 = r2.wrapping_add(r10); // 06037170 add r10,r2
                pc = if take { 0x06037186 } else { 0x06037172 };
            }
            0x06037172 => {
                r3 = cpu.read16(r7) as i16 as i32 as u32; r7 = r7.wrapping_add(2); // 06037172 mov.w @r7+,r3
                r3 <<= 2; // 06037174 shll2 r3
                r4 = cpu.read16(r7) as i16 as i32 as u32; r7 = r7.wrapping_add(2); // 06037176 mov.w @r7+,r4
                r3 <<= 2; // 06037178 shll2 r3
                r3 = r3.wrapping_add(r10); // 0603717a add r10,r3
                r4 <<= 2; // 0603717c shll2 r4
                r0 = cpu.read32(r9); r9 = r9.wrapping_add(4); // 0603717e mov.l @r9+,r0
                r4 <<= 2; // 06037180 shll2 r4
                let to = r0; // 06037182 jmp @r0
                r4 = r4.wrapping_add(r10); // 06037184 add r10,r4
                pc = to;
            }
            0x06037186 => {
                // 06037186 bra 0x6037088
                r7 = r7.wrapping_add(4i32 as u32); // 06037188 add #4,r7
                pc = 0x06037088;
            }
            0x0603718c => {
                r3 = cpu.read16(r7) as i16 as i32 as u32; r7 = r7.wrapping_add(2); // 0603718c mov.w @r7+,r3
                r2 <<= 2; // 0603718e shll2 r2
                r14 = cpu.read32(r1.wrapping_add(12)); // 06037190 mov.l @(12,r1),r14
                r2 = r2.wrapping_add(r10); // 06037192 add r10,r2
                r4 = cpu.read16(r7) as i16 as i32 as u32; r7 = r7.wrapping_add(2); // 06037194 mov.w @r7+,r4
                r0 = cpu.read32(r2.wrapping_add(12)); // 06037196 mov.l @(12,r2),r0
                r3 <<= 2; // 06037198 shll2 r3
                r14 = r14.wrapping_add(r0); // 0603719a add r0,r14
                r3 <<= 2; // 0603719c shll2 r3
                r3 = r3.wrapping_add(r10); // 0603719e add r10,r3
                r4 <<= 2; // 060371a0 shll2 r4
                r5 = cpu.read32(r3.wrapping_add(12)); // 060371a2 mov.l @(12,r3),r5
                r4 <<= 2; // 060371a4 shll2 r4
                r4 = r4.wrapping_add(r10); // 060371a6 add r10,r4
                r0 = cpu.read32(r4.wrapping_add(12)); // 060371a8 mov.l @(12,r4),r0
                r5 = r5.wrapping_add(r0); // 060371aa add r0,r5
                t = r14 & 1 != 0; r14 = (r14 as i32 >> 1) as u32; // 060371ac shar r14
                t = r5 & 1 != 0; r5 = (r5 as i32 >> 1) as u32; // 060371ae shar r5
                r14 = r14.wrapping_add(r5); // 060371b0 add r5,r14
                t = r14 & 1 != 0; r14 = (r14 as i32 >> 1) as u32; // 060371b2 shar r14
                r5 = cpu.read32(r15.wrapping_add(12)); // 060371b4 mov.l @(12,r15),r5
                r0 = cpu.read16(w.wrapping_add(160)) as i16 as i32 as u32; // 060371b6 mov.w @(160,gbr),r0
                r5 <<= 16; // 060371b8 shll16 r5
                r0 <<= 16; // 060371ba shll16 r0
                r0 = r0.wrapping_add(r5); // 060371bc add r5,r0
                t = r14 > r0; // 060371be cmp/hi r0,r14
                let take = t; // 060371c0 bt/s 0x6037292
                t = r5 & 1 != 0; r5 >>= 1; // 060371c2 shlr r5
                pc = if take { 0x06037292 } else { 0x060371c4 };
            }
            0x060371c4 => {
                t = r14 as i32 > r5 as i32; // 060371c4 cmp/gt r5,r14
                pc = if !t { 0x06037292 } else { 0x060371c8 }; // 060371c6 bf 0x6037292
            }
            0x060371c8 => {
                // 060371c8 bra 0x6037296
                r14 >>= 16; // 060371ca shlr16 r14
                pc = 0x06037296;
            }
            0x060371d4 => {
                r14 = cpu.read32(r1.wrapping_add(12)); // 060371d4 mov.l @(12,r1),r14
                r2 <<= 2; // 060371d6 shll2 r2
                r3 = cpu.read16(r7) as i16 as i32 as u32; r7 = r7.wrapping_add(2); // 060371d8 mov.w @r7+,r3
                r4 = cpu.read16(r7) as i16 as i32 as u32; r7 = r7.wrapping_add(2); // 060371da mov.w @r7+,r4
                r2 = r2.wrapping_add(r10); // 060371dc add r10,r2
                r0 = cpu.read32(r2.wrapping_add(12)); // 060371de mov.l @(12,r2),r0
                r3 <<= 2; // 060371e0 shll2 r3
                t = r14 as i32 > r0 as i32; // 060371e2 cmp/gt r0,r14
                let take = !t; // 060371e4 bf/s 0x60371ea
                r3 <<= 2; // 060371e6 shll2 r3
                pc = if take { 0x060371ea } else { 0x060371e8 };
            }
            0x060371e8 => {
                r14 = r0; // 060371e8 mov r0,r14
                pc = 0x060371ea;
            }
            0x060371ea => {
                r3 = r3.wrapping_add(r10); // 060371ea add r10,r3
                r4 <<= 2; // 060371ec shll2 r4
                r0 = cpu.read32(r3.wrapping_add(12)); // 060371ee mov.l @(12,r3),r0
                r4 <<= 2; // 060371f0 shll2 r4
                t = r14 as i32 > r0 as i32; // 060371f2 cmp/gt r0,r14
                let take = !t; // 060371f4 bf/s 0x60371fa
                r4 = r4.wrapping_add(r10); // 060371f6 add r10,r4
                pc = if take { 0x060371fa } else { 0x060371f8 };
            }
            0x060371f8 => {
                r14 = r0; // 060371f8 mov r0,r14
                pc = 0x060371fa;
            }
            0x060371fa => {
                r0 = cpu.read32(r4.wrapping_add(12)); // 060371fa mov.l @(12,r4),r0
                r5 = cpu.read32(r15.wrapping_add(12)); // 060371fc mov.l @(12,r15),r5
                t = r14 as i32 > r0 as i32; // 060371fe cmp/gt r0,r14
                pc = if !t { 0x06037204 } else { 0x06037202 }; // 06037200 bf 0x6037204
            }
            0x06037202 => {
                r14 = r0; // 06037202 mov r0,r14
                pc = 0x06037204;
            }
            0x06037204 => {
                r0 = cpu.read16(w.wrapping_add(160)) as i16 as i32 as u32; // 06037204 mov.w @(160,gbr),r0
                r5 <<= 16; // 06037206 shll16 r5
                r0 <<= 16; // 06037208 shll16 r0
                r0 = r0.wrapping_add(r5); // 0603720a add r5,r0
                t = r14 > r0; // 0603720c cmp/hi r0,r14
                let take = t; // 0603720e bt/s 0x6037292
                t = r5 & 1 != 0; r5 >>= 1; // 06037210 shlr r5
                pc = if take { 0x06037292 } else { 0x06037212 };
            }
            0x06037212 => {
                t = r14 as i32 > r5 as i32; // 06037212 cmp/gt r5,r14
                pc = if !t { 0x06037292 } else { 0x06037216 }; // 06037214 bf 0x6037292
            }
            0x06037216 => {
                // 06037216 bra 0x6037296
                r14 >>= 16; // 06037218 shlr16 r14
                pc = 0x06037296;
            }
            0x06037220 => {
                r3 = cpu.read16(r7) as i16 as i32 as u32; r7 = r7.wrapping_add(2); // 06037220 mov.w @r7+,r3
                r2 <<= 2; // 06037222 shll2 r2
                r14 = cpu.read32(r1.wrapping_add(12)); // 06037224 mov.l @(12,r1),r14
                r2 = r2.wrapping_add(r10); // 06037226 add r10,r2
                r4 = cpu.read16(r7) as i16 as i32 as u32; r7 = r7.wrapping_add(2); // 06037228 mov.w @r7+,r4
                r0 = cpu.read32(r2.wrapping_add(12)); // 0603722a mov.l @(12,r2),r0
                r3 <<= 2; // 0603722c shll2 r3
                r3 <<= 2; // 0603722e shll2 r3
                r3 = r3.wrapping_add(r10); // 06037230 add r10,r3
                t = r14 as i32 >= r0 as i32; // 06037232 cmp/ge r0,r14
                let take = t; // 06037234 bt/s 0x603723a
                r4 <<= 2; // 06037236 shll2 r4
                pc = if take { 0x0603723a } else { 0x06037238 };
            }
            0x06037238 => {
                r14 = r0; // 06037238 mov r0,r14
                pc = 0x0603723a;
            }
            0x0603723a => {
                r0 = cpu.read32(r3.wrapping_add(12)); // 0603723a mov.l @(12,r3),r0
                r4 <<= 2; // 0603723c shll2 r4
                t = r14 as i32 >= r0 as i32; // 0603723e cmp/ge r0,r14
                let take = t; // 06037240 bt/s 0x6037246
                r4 = r4.wrapping_add(r10); // 06037242 add r10,r4
                pc = if take { 0x06037246 } else { 0x06037244 };
            }
            0x06037244 => {
                r14 = r0; // 06037244 mov r0,r14
                pc = 0x06037246;
            }
            0x06037246 => {
                r0 = cpu.read32(r4.wrapping_add(12)); // 06037246 mov.l @(12,r4),r0
                r5 = cpu.read32(r15.wrapping_add(12)); // 06037248 mov.l @(12,r15),r5
                t = r14 as i32 >= r0 as i32; // 0603724a cmp/ge r0,r14
                pc = if t { 0x06037250 } else { 0x0603724e }; // 0603724c bt 0x6037250
            }
            0x0603724e => {
                r14 = r0; // 0603724e mov r0,r14
                pc = 0x06037250;
            }
            0x06037250 => {
                r0 = cpu.read16(w.wrapping_add(160)) as i16 as i32 as u32; // 06037250 mov.w @(160,gbr),r0
                r5 <<= 16; // 06037252 shll16 r5
                r0 <<= 16; // 06037254 shll16 r0
                r0 = r0.wrapping_add(r5); // 06037256 add r5,r0
                t = r14 > r0; // 06037258 cmp/hi r0,r14
                let take = t; // 0603725a bt/s 0x6037292
                t = r5 & 1 != 0; r5 >>= 1; // 0603725c shlr r5
                pc = if take { 0x06037292 } else { 0x0603725e };
            }
            0x0603725e => {
                t = r14 as i32 > r5 as i32; // 0603725e cmp/gt r5,r14
                pc = if !t { 0x06037292 } else { 0x06037262 }; // 06037260 bf 0x6037292
            }
            0x06037262 => {
                // 06037262 bra 0x6037296
                r14 >>= 16; // 06037264 shlr16 r14
                pc = 0x06037296;
            }
            0x0603726c => {
                r0 = cpu.read16(w.wrapping_add(160)) as i16 as i32 as u32; // 0603726c mov.w @(160,gbr),r0
                r2 <<= 2; // 0603726e shll2 r2
                t = r14 > r0; // 06037270 cmp/hi r0,r14
                let take = t; // 06037272 bt/s 0x603728a
                r2 = r2.wrapping_add(r10); // 06037274 add r10,r2
                pc = if take { 0x0603728a } else { 0x06037276 };
            }
            0x06037276 => {
                r3 = cpu.read16(r7) as i16 as i32 as u32; r7 = r7.wrapping_add(2); // 06037276 mov.w @r7+,r3
                r3 <<= 2; // 06037278 shll2 r3
                r4 = cpu.read16(r7) as i16 as i32 as u32; r7 = r7.wrapping_add(2); // 0603727a mov.w @r7+,r4
                r3 <<= 2; // 0603727c shll2 r3
                r3 = r3.wrapping_add(r10); // 0603727e add r10,r3
                r4 <<= 2; // 06037280 shll2 r4
                r0 = cpu.read32(r9); r9 = r9.wrapping_add(4); // 06037282 mov.l @r9+,r0
                r4 <<= 2; // 06037284 shll2 r4
                let to = r0; // 06037286 jmp @r0
                r4 = r4.wrapping_add(r10); // 06037288 add r10,r4
                pc = to;
            }
            0x0603728a => {
                // 0603728a bra 0x6037088
                r7 = r7.wrapping_add(4i32 as u32); // 0603728c add #4,r7
                pc = 0x06037088;
            }
            0x06037290 => {
                r14 = cpu.read32(r15); r15 = r15.wrapping_add(4); // 06037290 mov.l @r15+,r14
                pc = 0x06037292;
            }
            0x06037292 => {
                // 06037292 bra 0x6037088
                // nop // 06037294 nop 
                pc = 0x06037088;
            }
            0x06037296 => {
                r0 = cpu.read32(r1.wrapping_add(4)); // 06037296 mov.l @(4,r1),r0
                t = r0 & 1 != 0; r0 = (r0 as i32 >> 1) as u32; // 06037298 shar r0
                let take = !t; // 0603729a bf/s 0x60372ae
                r0 = cpu.read32(r2.wrapping_add(4)); // 0603729c mov.l @(4,r2),r0
                pc = if take { 0x060372ae } else { 0x0603729e };
            }
            0x0603729e => {
                t = r0 & 1 != 0; r0 = (r0 as i32 >> 1) as u32; // 0603729e shar r0
                let take = !t; // 060372a0 bf/s 0x60372ae
                r0 = cpu.read32(r3.wrapping_add(4)); // 060372a2 mov.l @(4,r3),r0
                pc = if take { 0x060372ae } else { 0x060372a4 };
            }
            0x060372a4 => {
                t = r0 & 1 != 0; r0 = (r0 as i32 >> 1) as u32; // 060372a4 shar r0
                let take = !t; // 060372a6 bf/s 0x60372ae
                r0 = cpu.read32(r4.wrapping_add(4)); // 060372a8 mov.l @(4,r4),r0
                pc = if take { 0x060372ae } else { 0x060372aa };
            }
            0x060372aa => {
                t = r0 & 1 != 0; r0 = (r0 as i32 >> 1) as u32; // 060372aa shar r0
                pc = if t { 0x06037292 } else { 0x060372ae }; // 060372ac bt 0x6037292
            }
            0x060372ae => {
                r0 = cpu.read32(w.wrapping_add(892)); // 060372ae mov.l @(892,gbr),r0
                t = r0 & 1 != 0; r0 = r0.rotate_right(1); // 060372b0 rotr r0
                pc = if t { 0x0603730c } else { 0x060372b4 }; // 060372b2 bt 0x603730c
            }
            0x060372b4 => {
                r15 = r15.wrapping_sub(4); cpu.write32(r15, r14); // 060372b4 mov.l r14,@-r15
                r14 = 0xfffffc00; // 060372b6 mov.w 0x6037314,r14
                r5 = 0x000003ff; // 060372b8 mov.w 0x6037316,r5
                r0 = cpu.read16(r1.wrapping_add(0)) as i16 as i32 as u32; // 060372ba mov.w @(0,r1),r0
                t = r0 as i32 > r5 as i32; // 060372bc cmp/gt r5,r0
                let take = t; // 060372be bt/s 0x6037290
                t = r0 as i32 >= r14 as i32; // 060372c0 cmp/ge r14,r0
                pc = if take { 0x06037290 } else { 0x060372c2 };
            }
            0x060372c2 => {
                let take = !t; // 060372c2 bf/s 0x6037290
                r0 = cpu.read16(r1.wrapping_add(2)) as i16 as i32 as u32; // 060372c4 mov.w @(2,r1),r0
                pc = if take { 0x06037290 } else { 0x060372c6 };
            }
            0x060372c6 => {
                t = r0 as i32 > r5 as i32; // 060372c6 cmp/gt r5,r0
                let take = t; // 060372c8 bt/s 0x6037290
                t = r0 as i32 >= r14 as i32; // 060372ca cmp/ge r14,r0
                pc = if take { 0x06037290 } else { 0x060372cc };
            }
            0x060372cc => {
                let take = !t; // 060372cc bf/s 0x6037290
                r0 = cpu.read16(r2.wrapping_add(0)) as i16 as i32 as u32; // 060372ce mov.w @(0,r2),r0
                pc = if take { 0x06037290 } else { 0x060372d0 };
            }
            0x060372d0 => {
                t = r0 as i32 > r5 as i32; // 060372d0 cmp/gt r5,r0
                let take = t; // 060372d2 bt/s 0x6037290
                t = r0 as i32 >= r14 as i32; // 060372d4 cmp/ge r14,r0
                pc = if take { 0x06037290 } else { 0x060372d6 };
            }
            0x060372d6 => {
                let take = !t; // 060372d6 bf/s 0x6037290
                r0 = cpu.read16(r2.wrapping_add(2)) as i16 as i32 as u32; // 060372d8 mov.w @(2,r2),r0
                pc = if take { 0x06037290 } else { 0x060372da };
            }
            0x060372da => {
                t = r0 as i32 > r5 as i32; // 060372da cmp/gt r5,r0
                let take = t; // 060372dc bt/s 0x6037290
                t = r0 as i32 >= r14 as i32; // 060372de cmp/ge r14,r0
                pc = if take { 0x06037290 } else { 0x060372e0 };
            }
            0x060372e0 => {
                let take = !t; // 060372e0 bf/s 0x6037290
                r0 = cpu.read16(r3.wrapping_add(0)) as i16 as i32 as u32; // 060372e2 mov.w @(0,r3),r0
                pc = if take { 0x06037290 } else { 0x060372e4 };
            }
            0x060372e4 => {
                t = r0 as i32 > r5 as i32; // 060372e4 cmp/gt r5,r0
                let take = t; // 060372e6 bt/s 0x6037290
                t = r0 as i32 >= r14 as i32; // 060372e8 cmp/ge r14,r0
                pc = if take { 0x06037290 } else { 0x060372ea };
            }
            0x060372ea => {
                let take = !t; // 060372ea bf/s 0x6037290
                r0 = cpu.read16(r3.wrapping_add(2)) as i16 as i32 as u32; // 060372ec mov.w @(2,r3),r0
                pc = if take { 0x06037290 } else { 0x060372ee };
            }
            0x060372ee => {
                t = r0 as i32 > r5 as i32; // 060372ee cmp/gt r5,r0
                let take = t; // 060372f0 bt/s 0x6037290
                t = r0 as i32 >= r14 as i32; // 060372f2 cmp/ge r14,r0
                pc = if take { 0x06037290 } else { 0x060372f4 };
            }
            0x060372f4 => {
                let take = !t; // 060372f4 bf/s 0x6037290
                r0 = cpu.read16(r4.wrapping_add(0)) as i16 as i32 as u32; // 060372f6 mov.w @(0,r4),r0
                pc = if take { 0x06037290 } else { 0x060372f8 };
            }
            0x060372f8 => {
                t = r0 as i32 > r5 as i32; // 060372f8 cmp/gt r5,r0
                let take = t; // 060372fa bt/s 0x6037290
                t = r0 as i32 >= r14 as i32; // 060372fc cmp/ge r14,r0
                pc = if take { 0x06037290 } else { 0x060372fe };
            }
            0x060372fe => {
                let take = !t; // 060372fe bf/s 0x6037290
                r0 = cpu.read16(r4.wrapping_add(2)) as i16 as i32 as u32; // 06037300 mov.w @(2,r4),r0
                pc = if take { 0x06037290 } else { 0x06037302 };
            }
            0x06037302 => {
                t = r0 as i32 > r5 as i32; // 06037302 cmp/gt r5,r0
                let take = t; // 06037304 bt/s 0x6037290
                t = r0 as i32 >= r14 as i32; // 06037306 cmp/ge r14,r0
                pc = if take { 0x06037290 } else { 0x06037308 };
            }
            0x06037308 => {
                pc = if !t { 0x06037290 } else { 0x0603730a }; // 06037308 bf 0x6037290
            }
            0x0603730a => {
                r14 = cpu.read32(r15); r15 = r15.wrapping_add(4); // 0603730a mov.l @r15+,r14
                pc = 0x0603730c;
            }
            0x0603730c => {
                r0 = cpu.read32(r9); r9 = r9.wrapping_add(4); // 0603730c mov.l @r9+,r0
                let to = r0; // 0603730e jmp @r0
                // nop // 06037310 nop 
                pc = to;
            }
            0x06037b18 => {
                r0 = cpu.read16(r8.wrapping_add(2)) as i16 as i32 as u32; // 06037b18 mov.w @(2,r8),r0
                r5 = cpu.read32(r15.wrapping_add(20)); // 06037b1a mov.l @(20,r15),r5
                r0 <<= 2; // 06037b1c shll2 r0
                r0 = r0.wrapping_add(r0); // 06037b1e add r0,r0
                r0 = cpu.read32(r0.wrapping_add(r5)); // 06037b20 mov.l @(r0,r5),r0
                cpu.write32(r11.wrapping_add(8), r0); // 06037b22 mov.l r0,@(8,r11)
                pc = 0x06037b24;
            }
            0x06037b24 => {
                r0 = cpu.read16(r8.wrapping_add(8)) as i16 as i32 as u32; // 06037b24 mov.w @(8,r8),r0
                r5 = cpu.read32(r8.wrapping_add(4)); // 06037b26 mov.l @(4,r8),r5
                cpu.write16(r11.wrapping_add(28), r0 as u16); // 06037b28 mov.w r0,@(28,r11)
                cpu.write32(r11.wrapping_add(4), r5); // 06037b2a mov.l r5,@(4,r11)
                pc = 0x06037b2c;
            }
            0x06037b2c => {
                r5 >>= 16; // 06037b2c shlr16 r5
                r5 >>= 2; // 06037b2e shlr2 r5
                t = r5 & 1 != 0; r5 >>= 1; // 06037b30 shlr r5
                pc = if !t { 0x06037b3a } else { 0x06037b34 }; // 06037b32 bf 0x6037b3a
            }
            0x06037b34 => {
                r1 = cpu.read32(r1.wrapping_add(0)); // 06037b34 mov.l @(0,r1),r1
                // 06037b36 bra 0x6037bdc
                r5 = cpu.read32(r2.wrapping_add(0)); // 06037b38 mov.l @(0,r2),r5
                pc = 0x06037bdc;
            }
            0x06037b3a => {
                r0 = cpu.read32(r1.wrapping_add(4)); // 06037b3a mov.l @(4,r1),r0
                r1 = cpu.read32(r1.wrapping_add(0)); // 06037b3c mov.l @(0,r1),r1
                t = r0 & 1 == 0; // 06037b3e tst #1,r0
                let take = t; // 06037b40 bt/s 0x6037bdc
                r5 = cpu.read32(r2.wrapping_add(0)); // 06037b42 mov.l @(0,r2),r5
                pc = if take { 0x06037bdc } else { 0x06037b44 };
            }
            0x06037b44 => {
                r0 = cpu.read32(r2.wrapping_add(4)); // 06037b44 mov.l @(4,r2),r0
                r2 = r5; // 06037b46 mov r5,r2
                t = r0 & 1 == 0; // 06037b48 tst #1,r0
                let take = t; // 06037b4a bt/s 0x6037bf2
                r5 = cpu.read32(r3.wrapping_add(0)); // 06037b4c mov.l @(0,r3),r5
                pc = if take { 0x06037bf2 } else { 0x06037b4e };
            }
            0x06037b4e => {
                r0 = cpu.read32(r3.wrapping_add(4)); // 06037b4e mov.l @(4,r3),r0
                r3 = r5; // 06037b50 mov r5,r3
                t = r0 & 1 == 0; // 06037b52 tst #1,r0
                let take = t; // 06037b54 bt/s 0x6037c08
                r5 = cpu.read32(r4.wrapping_add(0)); // 06037b56 mov.l @(0,r4),r5
                pc = if take { 0x06037c08 } else { 0x06037b58 };
            }
            0x06037b58 => {
                r0 = cpu.read32(r4.wrapping_add(4)); // 06037b58 mov.l @(4,r4),r0
                r4 = r5; // 06037b5a mov r5,r4
                t = r0 & 1 == 0; // 06037b5c tst #1,r0
                let take = t; // 06037b5e bt/s 0x6037c1e
                r5 = 1i32 as u32; // 06037b60 mov #1,r5
                pc = if take { 0x06037c1e } else { 0x06037b62 };
            }
            0x06037b62 => {
                r15 = r15.wrapping_sub(4); cpu.write32(r15, r9); // 06037b62 mov.l r9,@-r15
                r5 <<= 16; // 06037b64 shll16 r5
                t = r5 & 1 != 0; r5 >>= 1; // 06037b66 shlr r5
                r9 = r4.rotate_left(16); // 06037b68 swap.w r4,r9
                t = r5 & r9 == 0; // 06037b6a tst r5,r9
                { let (x, b1) = r13.overflowing_sub(r13); let (y, b2) = x.overflowing_sub(t as u32); r13 = y; t = b1 || b2; } // 06037b6c subc r13,r13
                r13 = !r13; // 06037b6e not r13,r13
                r13 ^= r9; // 06037b70 xor r9,r13
                t = r5 & r4 == 0; // 06037b72 tst r5,r4
                { let (x, b1) = r9.overflowing_sub(r9); let (y, b2) = x.overflowing_sub(t as u32); r9 = y; t = b1 || b2; } // 06037b74 subc r9,r9
                r9 = !r9; // 06037b76 not r9,r9
                r9 ^= r4; // 06037b78 xor r4,r9
                r13 = r13.wrapping_add(r9); // 06037b7a add r9,r13
                r13 = r13 & 0xffff; // 06037b7c extu.w r13,r13
                r0 = 0x06037c20; // 06037b7e mova 0x6037c20,r0
                r9 = r3.rotate_left(16); // 06037b80 swap.w r3,r9
                t = r5 & r9 == 0; // 06037b82 tst r5,r9
                { let (x, b1) = r12.overflowing_sub(r12); let (y, b2) = x.overflowing_sub(t as u32); r12 = y; t = b1 || b2; } // 06037b84 subc r12,r12
                r12 = !r12; // 06037b86 not r12,r12
                r12 ^= r9; // 06037b88 xor r9,r12
                t = r5 & r3 == 0; // 06037b8a tst r5,r3
                { let (x, b1) = r9.overflowing_sub(r9); let (y, b2) = x.overflowing_sub(t as u32); r9 = y; t = b1 || b2; } // 06037b8c subc r9,r9
                r9 = !r9; // 06037b8e not r9,r9
                r9 ^= r3; // 06037b90 xor r3,r9
                r12 = r12.wrapping_add(r9); // 06037b92 add r9,r12
                r12 = r12 & 0xffff; // 06037b94 extu.w r12,r12
                t = r12 as i32 > r13 as i32; // 06037b96 cmp/gt r13,r12
                let take = t; // 06037b98 bt/s 0x6037ba0
                r9 = r2.rotate_left(16); // 06037b9a swap.w r2,r9
                pc = if take { 0x06037ba0 } else { 0x06037b9c };
            }
            0x06037b9c => {
                r13 = r12; // 06037b9c mov r12,r13
                r0 = 0x06037c0c; // 06037b9e mova 0x6037c0c,r0
                pc = 0x06037ba0;
            }
            0x06037ba0 => {
                t = r5 & r9 == 0; // 06037ba0 tst r5,r9
                { let (x, b1) = r12.overflowing_sub(r12); let (y, b2) = x.overflowing_sub(t as u32); r12 = y; t = b1 || b2; } // 06037ba2 subc r12,r12
                r12 = !r12; // 06037ba4 not r12,r12
                r12 ^= r9; // 06037ba6 xor r9,r12
                t = r5 & r2 == 0; // 06037ba8 tst r5,r2
                { let (x, b1) = r9.overflowing_sub(r9); let (y, b2) = x.overflowing_sub(t as u32); r9 = y; t = b1 || b2; } // 06037baa subc r9,r9
                r9 = !r9; // 06037bac not r9,r9
                r9 ^= r2; // 06037bae xor r2,r9
                r12 = r12.wrapping_add(r9); // 06037bb0 add r9,r12
                r12 = r12 & 0xffff; // 06037bb2 extu.w r12,r12
                t = r12 as i32 > r13 as i32; // 06037bb4 cmp/gt r13,r12
                let take = t; // 06037bb6 bt/s 0x6037bbe
                r9 = r1.rotate_left(16); // 06037bb8 swap.w r1,r9
                pc = if take { 0x06037bbe } else { 0x06037bba };
            }
            0x06037bba => {
                r13 = r12; // 06037bba mov r12,r13
                r0 = 0x06037bf8; // 06037bbc mova 0x6037bf8,r0
                pc = 0x06037bbe;
            }
            0x06037bbe => {
                t = r5 & r9 == 0; // 06037bbe tst r5,r9
                { let (x, b1) = r12.overflowing_sub(r12); let (y, b2) = x.overflowing_sub(t as u32); r12 = y; t = b1 || b2; } // 06037bc0 subc r12,r12
                r12 = !r12; // 06037bc2 not r12,r12
                r12 ^= r9; // 06037bc4 xor r9,r12
                t = r5 & r1 == 0; // 06037bc6 tst r5,r1
                { let (x, b1) = r9.overflowing_sub(r9); let (y, b2) = x.overflowing_sub(t as u32); r9 = y; t = b1 || b2; } // 06037bc8 subc r9,r9
                r9 = !r9; // 06037bca not r9,r9
                r9 ^= r1; // 06037bcc xor r1,r9
                r12 = r12.wrapping_add(r9); // 06037bce add r9,r12
                r12 = r12 & 0xffff; // 06037bd0 extu.w r12,r12
                t = r13 as i32 >= r12 as i32; // 06037bd2 cmp/ge r12,r13
                let take = t; // 06037bd4 bt/s 0x6037be2
                r9 = cpu.read32(r15); r15 = r15.wrapping_add(4); // 06037bd6 mov.l @r15+,r9
                pc = if take { 0x06037be2 } else { 0x06037bd8 };
            }
            0x06037bd8 => {
                let to = r0; // 06037bd8 jmp @r0
                r5 = cpu.read32(r9); r9 = r9.wrapping_add(4); // 06037bda mov.l @r9+,r5
                pc = to;
            }
            0x06037bdc => {
                r3 = cpu.read32(r3.wrapping_add(0)); // 06037bdc mov.l @(0,r3),r3
                r2 = r5; // 06037bde mov r5,r2
                r4 = cpu.read32(r4.wrapping_add(0)); // 06037be0 mov.l @(0,r4),r4
                pc = 0x06037be2;
            }
            0x06037be2 => {
                r5 = cpu.read32(r9); r9 = r9.wrapping_add(4); // 06037be2 mov.l @r9+,r5
                cpu.write32(r11.wrapping_add(12), r1); // 06037be4 mov.l r1,@(12,r11)
                cpu.write32(r11.wrapping_add(16), r2); // 06037be6 mov.l r2,@(16,r11)
                cpu.write32(r11.wrapping_add(20), r3); // 06037be8 mov.l r3,@(20,r11)
                let to = r5; // 06037bea jmp @r5
                cpu.write32(r11.wrapping_add(24), r4); // 06037bec mov.l r4,@(24,r11)
                pc = to;
            }
            0x06037bf2 => {
                r4 = cpu.read32(r4.wrapping_add(0)); // 06037bf2 mov.l @(0,r4),r4
                r3 = r5; // 06037bf4 mov r5,r3
                r5 = cpu.read32(r9); r9 = r9.wrapping_add(4); // 06037bf6 mov.l @r9+,r5
                pc = 0x06037bf8;
            }
            0x06037bf8 => {
                r0 = cpu.read16(r11.wrapping_add(0)) as i16 as i32 as u32; // 06037bf8 mov.w @(0,r11),r0
                cpu.write32(r11.wrapping_add(16), r1); // 06037bfa mov.l r1,@(16,r11)
                r0 ^= 16; // 06037bfc xor #16,r0
                cpu.write16(r11.wrapping_add(0), r0 as u16); // 06037bfe mov.w r0,@(0,r11)
                cpu.write32(r11.wrapping_add(12), r2); // 06037c00 mov.l r2,@(12,r11)
                cpu.write32(r11.wrapping_add(24), r3); // 06037c02 mov.l r3,@(24,r11)
                let to = r5; // 06037c04 jmp @r5
                cpu.write32(r11.wrapping_add(20), r4); // 06037c06 mov.l r4,@(20,r11)
                pc = to;
            }
            0x06037c08 => {
                r4 = r5; // 06037c08 mov r5,r4
                r5 = cpu.read32(r9); r9 = r9.wrapping_add(4); // 06037c0a mov.l @r9+,r5
                pc = 0x06037c0c;
            }
            0x06037c0c => {
                r0 = cpu.read16(r11.wrapping_add(0)) as i16 as i32 as u32; // 06037c0c mov.w @(0,r11),r0
                cpu.write32(r11.wrapping_add(20), r1); // 06037c0e mov.l r1,@(20,r11)
                r0 ^= 48; // 06037c10 xor #48,r0
                cpu.write16(r11.wrapping_add(0), r0 as u16); // 06037c12 mov.w r0,@(0,r11)
                cpu.write32(r11.wrapping_add(24), r2); // 06037c14 mov.l r2,@(24,r11)
                cpu.write32(r11.wrapping_add(12), r3); // 06037c16 mov.l r3,@(12,r11)
                let to = r5; // 06037c18 jmp @r5
                cpu.write32(r11.wrapping_add(16), r4); // 06037c1a mov.l r4,@(16,r11)
                pc = to;
            }
            0x06037c1e => {
                r5 = cpu.read32(r9); r9 = r9.wrapping_add(4); // 06037c1e mov.l @r9+,r5
                pc = 0x06037c20;
            }
            0x06037c20 => {
                r0 = cpu.read16(r11.wrapping_add(0)) as i16 as i32 as u32; // 06037c20 mov.w @(0,r11),r0
                cpu.write32(r11.wrapping_add(24), r1); // 06037c22 mov.l r1,@(24,r11)
                r0 ^= 32; // 06037c24 xor #32,r0
                cpu.write16(r11.wrapping_add(0), r0 as u16); // 06037c26 mov.w r0,@(0,r11)
                cpu.write32(r11.wrapping_add(20), r2); // 06037c28 mov.l r2,@(20,r11)
                cpu.write32(r11.wrapping_add(16), r3); // 06037c2a mov.l r3,@(16,r11)
                let to = r5; // 06037c2c jmp @r5
                cpu.write32(r11.wrapping_add(12), r4); // 06037c2e mov.l r4,@(12,r11)
                pc = to;
            }
            0x06037c30 => {
                mac = 0; // 06037c30 clrmac 
                r5 = w; // 06037c32 stc gbr,r5
                r4 = r15; // 06037c34 mov r15,r4
                r5 = r5.wrapping_add(88i32 as u32); // 06037c36 add #88,r5
                { let x = cpu.read32(r4) as i32 as i64; r4 = r4.wrapping_add(4); let y = cpu.read32(r5) as i32 as i64; r5 = r5.wrapping_add(4); mac = mac.wrapping_add(x * y); if cpu.sh2.sr & 2 != 0 { mac = mac.clamp(-0x8000_0000_0000, 0x7fff_ffff_ffff); } } // 06037c38 mac.l @r4+,@r5+
                r0 = cpu.read16(r8.wrapping_add(6)) as i16 as i32 as u32; // 06037c3a mov.w @(6,r8),r0
                r1 = 31i32 as u32; // 06037c3c mov #31,r1
                r3 = (r0 & 0xffff_0000) | ((r0 & 0xff) << 8) | ((r0 >> 8) & 0xff); // 06037c3e swap.b r0,r3
                { let x = cpu.read32(r4) as i32 as i64; r4 = r4.wrapping_add(4); let y = cpu.read32(r5) as i32 as i64; r5 = r5.wrapping_add(4); mac = mac.wrapping_add(x * y); if cpu.sh2.sr & 2 != 0 { mac = mac.clamp(-0x8000_0000_0000, 0x7fff_ffff_ffff); } } // 06037c40 mac.l @r4+,@r5+
                r3 >>= 2; // 06037c42 shlr2 r3
                r3 &= r1; // 06037c44 and r1,r3
                r1 &= r0; // 06037c46 and r0,r1
                { let x = cpu.read32(r4) as i32 as i64; r4 = r4.wrapping_add(4); let y = cpu.read32(r5) as i32 as i64; r5 = r5.wrapping_add(4); mac = mac.wrapping_add(x * y); if cpu.sh2.sr & 2 != 0 { mac = mac.clamp(-0x8000_0000_0000, 0x7fff_ffff_ffff); } } // 06037c48 mac.l @r4+,@r5+
                r0 >>= 2; // 06037c4a shlr2 r0
                r0 >>= 2; // 06037c4c shlr2 r0
                r2 = 31i32 as u32; // 06037c4e mov #31,r2
                t = r0 & 1 != 0; r0 >>= 1; // 06037c50 shlr r0
                r2 &= r0; // 06037c52 and r0,r2
                r5 = (mac >> 32) as u32; // 06037c54 sts mach,r5
                r4 = mac as u32; // 06037c56 sts macl,r4
                t = r5 & r5 == 0; // 06037c58 tst r5,r5
                let take = t; // 06037c5a bt/s 0x6037c70
                r0 = cpu.read32(w.wrapping_add(100)); // 06037c5c mov.l @(100,gbr),r0
                pc = if take { 0x06037c70 } else { 0x06037c5e };
            }
            0x06037c5e => {
                r5 = r5.wrapping_add(1i32 as u32); // 06037c5e add #1,r5
                t = r5 & r5 == 0; // 06037c60 tst r5,r5
                pc = if t { 0x06037c74 } else { 0x06037c64 }; // 06037c62 bt 0x6037c74
            }
            0x06037c64 => {
                r4 = -1i32 as u32; // 06037c64 mov #-1,r4
                t = r5 as i32 > 0; // 06037c66 cmp/pl r5
                let take = t; // 06037c68 bt/s 0x6037c7c
                r4 = r4 & 0xff; // 06037c6a extu.b r4,r4
                pc = if take { 0x06037c7c } else { 0x06037c6c };
            }
            0x06037c6c => {
                // 06037c6c bra 0x6037c84
                // nop // 06037c6e nop 
                pc = 0x06037c84;
            }
            0x06037c70 => {
                r5 = -128i32 as u32; // 06037c70 mov #-128,r5
                r5 = r5 & 0xff; // 06037c72 extu.b r5,r5
                pc = 0x06037c74;
            }
            0x06037c74 => {
                r4 >>= 16; // 06037c74 shlr16 r4
                r4 >>= 8; // 06037c76 shlr8 r4
                t = r4 & 1 != 0; r4 >>= 1; // 06037c78 shlr r4
                r4 |= r5; // 06037c7a or r5,r4
                pc = 0x06037c7c;
            }
            0x06037c7c => {
                r5 = -8i32 as u32; // 06037c7c mov #-8,r5
                r4 &= r5; // 06037c7e and r5,r4
                r4 <<= 2; // 06037c80 shll2 r4
                r0 = r0.wrapping_add(r4); // 06037c82 add r4,r0
                pc = 0x06037c84;
            }
            0x06037c84 => {
                r5 = 4i32 as u32; // 06037c84 mov #4,r5
                r5 <<= 8; // 06037c86 shll8 r5
                r1 = cpu.read8(r0.wrapping_add(r1)) as i8 as i32 as u32; // 06037c88 mov.b @(r0,r1),r1
                r0 = r0.wrapping_add(r5); // 06037c8a add r5,r0
                r2 = cpu.read8(r0.wrapping_add(r2)) as i8 as i32 as u32; // 06037c8c mov.b @(r0,r2),r2
                r0 = r0.wrapping_add(r5); // 06037c8e add r5,r0
                r0 = cpu.read8(r0.wrapping_add(r3)) as i8 as i32 as u32; // 06037c90 mov.b @(r0,r3),r0
                r0 |= 32; // 06037c92 or #32,r0
                r0 = (r0 & 0xffff_0000) | ((r0 & 0xff) << 8) | ((r0 >> 8) & 0xff); // 06037c94 swap.b r0,r0
                r2 <<= 2; // 06037c96 shll2 r2
                t = r2 >> 31 != 0; r2 <<= 1; // 06037c98 shll r2
                r0 = r0.wrapping_add(r2); // 06037c9a add r2,r0
                r0 <<= 2; // 06037c9c shll2 r0
                r1 |= r0; // 06037c9e or r0,r1
                r0 = cpu.read16(r8.wrapping_add(4)) as i16 as i32 as u32; // 06037ca0 mov.w @(4,r8),r0
                r1 <<= 16; // 06037ca2 shll16 r1
                r1 = (r0 << 16) | (r1 >> 16); // 06037ca4 xtrct r0,r1
                r0 = cpu.read16(r8.wrapping_add(8)) as i16 as i32 as u32; // 06037ca6 mov.w @(8,r8),r0
                cpu.write32(r11.wrapping_add(4), r1); // 06037ca8 mov.l r1,@(4,r11)
                // 06037caa rts
                cpu.write16(r11.wrapping_add(28), r0 as u16); // 06037cac mov.w r0,@(28,r11)
                if pr == entry_pr { break; }
                pc = pr;
            }
            0x06037cae => {
                mac = 0; // 06037cae clrmac 
                r4 = r15; // 06037cb0 mov r15,r4
                r5 = w; // 06037cb2 stc gbr,r5
                r5 = r5.wrapping_add(88i32 as u32); // 06037cb4 add #88,r5
                { let x = cpu.read32(r4) as i32 as i64; r4 = r4.wrapping_add(4); let y = cpu.read32(r5) as i32 as i64; r5 = r5.wrapping_add(4); mac = mac.wrapping_add(x * y); if cpu.sh2.sr & 2 != 0 { mac = mac.clamp(-0x8000_0000_0000, 0x7fff_ffff_ffff); } } // 06037cb6 mac.l @r4+,@r5+
                { let x = cpu.read32(r4) as i32 as i64; r4 = r4.wrapping_add(4); let y = cpu.read32(r5) as i32 as i64; r5 = r5.wrapping_add(4); mac = mac.wrapping_add(x * y); if cpu.sh2.sr & 2 != 0 { mac = mac.clamp(-0x8000_0000_0000, 0x7fff_ffff_ffff); } } // 06037cb8 mac.l @r4+,@r5+
                r0 = cpu.read32(r8.wrapping_add(4)); // 06037cba mov.l @(4,r8),r0
                { let x = cpu.read32(r4) as i32 as i64; r4 = r4.wrapping_add(4); let y = cpu.read32(r5) as i32 as i64; r5 = r5.wrapping_add(4); mac = mac.wrapping_add(x * y); if cpu.sh2.sr & 2 != 0 { mac = mac.clamp(-0x8000_0000_0000, 0x7fff_ffff_ffff); } } // 06037cbc mac.l @r4+,@r5+
                r1 = 7i32 as u32; // 06037cbe mov #7,r1
                r5 = (mac >> 32) as u32; // 06037cc0 sts mach,r5
                r4 = mac as u32; // 06037cc2 sts macl,r4
                t = r5 & r5 == 0; // 06037cc4 tst r5,r5
                let take = t; // 06037cc6 bt/s 0x6037cd4
                r5 = r5.wrapping_add(1i32 as u32); // 06037cc8 add #1,r5
                pc = if take { 0x06037cd4 } else { 0x06037cca };
            }
            0x06037cca => {
                t = r5 & r5 == 0; // 06037cca tst r5,r5
                let take = t; // 06037ccc bt/s 0x6037cd6
                t = r5 as i32 >= 0; // 06037cce cmp/pz r5
                pc = if take { 0x06037cd6 } else { 0x06037cd0 };
            }
            0x06037cd0 => {
                let take = !t; // 06037cd0 bf/s 0x6037ce2
                r4 = -1i32 as u32; // 06037cd2 mov #-1,r4
                pc = if take { 0x06037ce2 } else { 0x06037cd4 };
            }
            0x06037cd4 => {
                r5 = 8i32 as u32; // 06037cd4 mov #8,r5
                pc = 0x06037cd6;
            }
            0x06037cd6 => {
                t = r4 >> 31 != 0; r4 = r4.rotate_left(1); // 06037cd6 rotl r4
                t = r4 >> 31 != 0; r4 = r4.rotate_left(1); // 06037cd8 rotl r4
                t = r4 >> 31 != 0; r4 = r4.rotate_left(1); // 06037cda rotl r4
                r4 &= r1; // 06037cdc and r1,r4
                r4 |= r5; // 06037cde or r5,r4
                r0 = r0.wrapping_add(r4); // 06037ce0 add r4,r0
                pc = 0x06037ce2;
            }
            0x06037ce2 => {
                // 06037ce2 rts
                cpu.write32(r11.wrapping_add(4), r0); // 06037ce4 mov.l r0,@(4,r11)
                if pr == entry_pr { break; }
                pc = pr;
            }
            0x06037ce6 => {
                r0 = cpu.read16(r8.wrapping_add(2)) as i16 as i32 as u32; // 06037ce6 mov.w @(2,r8),r0
                r1 = cpu.read32(r15.wrapping_add(20)); // 06037ce8 mov.l @(20,r15),r1
                r0 <<= 2; // 06037cea shll2 r0
                r0 = r0.wrapping_add(r0); // 06037cec add r0,r0
                r0 = cpu.read32(r0.wrapping_add(r1)); // 06037cee mov.l @(r0,r1),r0
                mac = 0; // 06037cf0 clrmac 
                cpu.write32(r11.wrapping_add(8), r0); // 06037cf2 mov.l r0,@(8,r11)
                r5 = w; // 06037cf4 stc gbr,r5
                r4 = r15; // 06037cf6 mov r15,r4
                r5 = r5.wrapping_add(88i32 as u32); // 06037cf8 add #88,r5
                { let x = cpu.read32(r4) as i32 as i64; r4 = r4.wrapping_add(4); let y = cpu.read32(r5) as i32 as i64; r5 = r5.wrapping_add(4); mac = mac.wrapping_add(x * y); if cpu.sh2.sr & 2 != 0 { mac = mac.clamp(-0x8000_0000_0000, 0x7fff_ffff_ffff); } } // 06037cfa mac.l @r4+,@r5+
                r3 = cpu.read32(r8.wrapping_add(4)); // 06037cfc mov.l @(4,r8),r3
                { let x = cpu.read32(r4) as i32 as i64; r4 = r4.wrapping_add(4); let y = cpu.read32(r5) as i32 as i64; r5 = r5.wrapping_add(4); mac = mac.wrapping_add(x * y); if cpu.sh2.sr & 2 != 0 { mac = mac.clamp(-0x8000_0000_0000, 0x7fff_ffff_ffff); } } // 06037cfe mac.l @r4+,@r5+
                r0 = r3.rotate_left(16); // 06037d00 swap.w r3,r0
                r0 &= 56; // 06037d02 and #56,r0
                { let x = cpu.read32(r4) as i32 as i64; r4 = r4.wrapping_add(4); let y = cpu.read32(r5) as i32 as i64; r5 = r5.wrapping_add(4); mac = mac.wrapping_add(x * y); if cpu.sh2.sr & 2 != 0 { mac = mac.clamp(-0x8000_0000_0000, 0x7fff_ffff_ffff); } } // 06037d04 mac.l @r4+,@r5+
                t = r0 == 8i32 as u32; // 06037d06 cmp/eq #8,r0
                pc = if t { 0x06037d0e } else { 0x06037d0a }; // 06037d08 bt 0x6037d0e
            }
            0x06037d0a => {
                t = r0 == 40i32 as u32; // 06037d0a cmp/eq #40,r0
                pc = if !t { 0x06037d42 } else { 0x06037d0e }; // 06037d0c bf 0x6037d42
            }
            0x06037d0e => {
                r0 = 0xffffffe0; // 06037d0e mov.w 0x6037d4a,r0
                r1 = (mac >> 32) as u32; // 06037d10 sts mach,r1
                r2 = mac as u32; // 06037d12 sts macl,r2
                t = r1 & r1 == 0; // 06037d14 tst r1,r1
                let take = t; // 06037d16 bt/s 0x6037d24
                r1 = r1.wrapping_add(1i32 as u32); // 06037d18 add #1,r1
                pc = if take { 0x06037d24 } else { 0x06037d1a };
            }
            0x06037d1a => {
                t = r1 & r1 == 0; // 06037d1a tst r1,r1
                let take = t; // 06037d1c bt/s 0x6037d26
                t = r1 as i32 >= 0; // 06037d1e cmp/pz r1
                pc = if take { 0x06037d26 } else { 0x06037d20 };
            }
            0x06037d20 => {
                let take = !t; // 06037d20 bf/s 0x6037d32
                r2 = -1i32 as u32; // 06037d22 mov #-1,r2
                pc = if take { 0x06037d32 } else { 0x06037d24 };
            }
            0x06037d24 => {
                r1 = 16i32 as u32; // 06037d24 mov #16,r1
                pc = 0x06037d26;
            }
            0x06037d26 => {
                r2 >>= 16; // 06037d26 shlr16 r2
                r2 >>= 8; // 06037d28 shlr8 r2
                r2 >>= 2; // 06037d2a shlr2 r2
                r2 >>= 2; // 06037d2c shlr2 r2
                r2 |= r1; // 06037d2e or r1,r2
                r0 = r0.wrapping_add(r2); // 06037d30 add r2,r0
                pc = 0x06037d32;
            }
            0x06037d32 => {
                r2 = -8i32 as u32; // 06037d32 mov #-8,r2
                cpu.write16(r11.wrapping_add(28), r0 as u16); // 06037d34 mov.w r0,@(28,r11)
                r0 = r3.rotate_left(16); // 06037d36 swap.w r3,r0
                r0 &= r2; // 06037d38 and r2,r0
                r0 |= 4; // 06037d3a or #4,r0
                r0 = r0.rotate_left(16); // 06037d3c swap.w r0,r0
                // 06037d3e rts
                cpu.write32(r11.wrapping_add(4), r0); // 06037d40 mov.l r0,@(4,r11)
                if pr == entry_pr { break; }
                pc = pr;
            }
            0x06037d42 => {
                r0 = cpu.read16(r8.wrapping_add(8)) as i16 as i32 as u32; // 06037d42 mov.w @(8,r8),r0
                cpu.write32(r11.wrapping_add(4), r3); // 06037d44 mov.l r3,@(4,r11)
                // 06037d46 rts
                cpu.write16(r11.wrapping_add(28), r0 as u16); // 06037d48 mov.w r0,@(28,r11)
                if pr == entry_pr { break; }
                pc = pr;
            }
            0x06037e64 => {
                r15 = r15.wrapping_sub(4); cpu.write32(r15, r2); // 06037e64 mov.l r2,@-r15
                r15 = r15.wrapping_sub(4); cpu.write32(r15, r1); // 06037e66 mov.l r1,@-r15
                r1 = 0x060383cc; // 06037e68 mov.l 0x6037ea8,r1
                r13 = cpu.read32(r1.wrapping_add(0)); // 06037e6a mov.l @(0,r1),r13
                r12 = cpu.read32(r1.wrapping_add(4)); // 06037e6c mov.l @(4,r1),r12
                r5 = r14; // 06037e6e mov r14,r5
                t = r13 as i32 >= r5 as i32; // 06037e70 cmp/ge r5,r13
                let take = t; // 06037e72 bt/s 0x6037e8e
                r0 = 0i32 as u32; // 06037e74 mov #0,r0
                pc = if take { 0x06037e8e } else { 0x06037e76 };
            }
            0x06037e76 => {
                r5 = r5.wrapping_sub(r13); // 06037e76 sub r13,r5
                t = r5 as i32 >= r12 as i32; // 06037e78 cmp/ge r12,r5
                let take = t; // 06037e7a bt/s 0x6037e8e
                r0 = cpu.read16(r1.wrapping_add(10)) as i16 as i32 as u32; // 06037e7c mov.w @(10,r1),r0
                pc = if take { 0x06037e8e } else { 0x06037e7e };
            }
            0x06037e7e => {
                r0 = cpu.read16(r1.wrapping_add(8)) as i16 as i32 as u32; // 06037e7e mov.w @(8,r1),r0
                r2 = r0; // 06037e80 mov r0,r2
                t = r0 == 0i32 as u32; // 06037e82 cmp/eq #0,r0
                let take = t; // 06037e84 bt/s 0x6037e8e
                r0 = r5; // 06037e86 mov r5,r0
                pc = if take { 0x06037e8e } else { 0x06037e88 };
            }
            0x06037e88 => {
                r2 = r2.wrapping_sub(1); t = r2 == 0; // 06037e88 dt r2
                let take = !t; // 06037e8a bf/s 0x6037e88
                t = r0 & 1 != 0; r0 >>= 1; // 06037e8c shlr r0
                pc = if take { 0x06037e88 } else { 0x06037e8e };
            }
            0x06037e8e => {
                r5 = r0; // 06037e8e mov r0,r5
                r0 = cpu.read16(r1.wrapping_add(12)) as i16 as i32 as u32; // 06037e90 mov.w @(12,r1),r0
                r0 = r0.wrapping_add(r5); // 06037e92 add r5,r0
                cpu.write16(r8.wrapping_add(8), r0 as u16); // 06037e94 mov.w r0,@(8,r8)
                r0 = cpu.read16(r8.wrapping_add(4)) as i16 as i32 as u32; // 06037e96 mov.w @(4,r8),r0
                r0 |= 4; // 06037e98 or #4,r0
                cpu.write16(r8.wrapping_add(4), r0 as u16); // 06037e9a mov.w r0,@(4,r8)
                r1 = cpu.read32(r15); r15 = r15.wrapping_add(4); // 06037e9c mov.l @r15+,r1
                r2 = cpu.read32(r15); r15 = r15.wrapping_add(4); // 06037e9e mov.l @r15+,r2
                r0 = 0x06037b2c; // 06037ea0 mov.l 0x6037eac,r0
                let to = r0; // 06037ea2 jmp @r0
                // nop // 06037ea4 nop 
                pc = to;
            }
            _ => unreachable!("{pc:#010x}"),
        }
    }
    hand_over!();
    cpu.leave_r14(r14);
}

/// 4: the frame's end on the slave: the depth table walked from the farthest list to the nearest
/// (128 lists), each list's commands linked in turn into VDP1's command list (SGL's, from
/// 0x060089bc: jumps to each command, the window's clipping commands where the drawing changes
/// window), the lists emptied. SGL's code block by block (asm2sm.py).
pub fn slave_frame_draw(cpu: &mut Cpu) {
    slave_handler_4_blocks(cpu);
}

#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
fn slave_handler_4_blocks(cpu: &mut Cpu) {
    let w = cpu.gbr();
    let [mut r0, mut r1, mut r2, mut r3, mut r4, mut r5, mut r6, mut r7, mut r8, mut r9, mut r10, mut r11, mut r12, mut r13, _, mut r15] = cpu.sh2.r;
    let mut r14 = cpu.caller_r14;
    let mut mac = ((cpu.sh2.mach as u64) << 32 | cpu.sh2.macl as u64) as i64;
    let mut t = cpu.sh2.sr & 1 != 0;
    let entry_pr = cpu.sh2.pr;
    let mut pr = entry_pr;
    macro_rules! hand_over { () => { cpu.sh2.r = [r0, r1, r2, r3, r4, r5, r6, r7, r8, r9, r10, r11, r12, r13, r14, r15]; cpu.leave_mac(mac); cpu.leave_t(t); } }
    macro_rules! take_back { () => { [r0, r1, r2, r3, r4, r5, r6, r7, r8, r9, r10, r11, r12, r13, r14, r15] = cpu.sh2.r; mac = ((cpu.sh2.mach as u64) << 32 | cpu.sh2.macl as u64) as i64; t = cpu.sh2.sr & 1 != 0; } }
    let mut pc: u32 = 0x06036ab0;
    loop {
        match pc {
            0x06036ab0 => {
                r10 = 0x060089c0; // 06036ab0 mov.l 0x6036bc4,r10
                r11 = 0x060089bc; // 06036ab2 mov.l 0x6036bc0,r11
                r10 = cpu.read32(r10); // 06036ab4 mov.l @r10,r10
                r12 = 0x00000400; // 06036ab6 mov.w 0x6036bba,r12
                r11 = cpu.read32(r11); // 06036ab8 mov.l @r11,r11
                r0 = 0x260ffcb0; // 06036aba mov.l 0x6036be0,r0
                r10 = r10.wrapping_add(r12); // 06036abc add r12,r10
                r0 = cpu.read8(r0) as i8 as i32 as u32; // 06036abe mov.b @r0,r0
                r12 = r10; // 06036ac0 mov r10,r12
                t = r0 & 128 == 0; // 06036ac2 tst #128,r0
                let take = t; // 06036ac4 bt/s 0x6036ace
                r10 = r10.wrapping_add(-4i32 as u32); // 06036ac6 add #-4,r10
                pc = if take { 0x06036ace } else { 0x06036ac8 };
            }
            0x06036ac8 => {
                r0 = 0x060364cc; // 06036ac8 mov.l 0x6036be4,r0
                cpu.write32(r11.wrapping_add(8), r0); // 06036aca mov.l r0,@(8,r11)
                r11 = r11.wrapping_add(12i32 as u32); // 06036acc add #12,r11
                pc = 0x06036ace;
            }
            0x06036ace => {
                r9 = 0x00000080; // 06036ace mov.w 0x6036bbc,r9
                r8 = cpu.read32(r10); // 06036ad0 mov.l @r10,r8
                pc = 0x06036ad2;
            }
            0x06036ad2 => {
                r10 = r10.wrapping_add(-4i32 as u32); // 06036ad2 add #-4,r10
                t = r8 & r8 == 0; // 06036ad4 tst r8,r8
                let take = !t; // 06036ad6 bf/s 0x6036afa
                r9 = r9.wrapping_sub(1); t = r9 == 0; // 06036ad8 dt r9
                pc = if take { 0x06036afa } else { 0x06036ada };
            }
            0x06036ada => {
                let take = !t; // 06036ada bf/s 0x6036ad2
                r8 = cpu.read32(r10); // 06036adc mov.l @r10,r8
                pc = if take { 0x06036ad2 } else { 0x06036ade };
            }
            0x06036ade => {
                r0 = cpu.read8(w.wrapping_add(169)) as i8 as i32 as u32; // 06036ade mov.b @(169,gbr),r0
                t = r0 & 32 == 0; // 06036ae0 tst #32,r0
                let take = t; // 06036ae2 bt/s 0x6036b5c
                r0 = 0i32 as u32; // 06036ae4 mov #0,r0
                pc = if take { 0x06036b5c } else { 0x06036ae6 };
            }
            0x06036ae6 => {
                cpu.write8(w.wrapping_add(169), r0 as u8); // 06036ae6 mov.b r0,@(169,gbr)
                r0 = r10; // 06036ae8 mov r10,r0
                r1 = 0x060fff00; // 06036aea mov.l 0x6036bcc,r1
                r0 = r0.wrapping_add(4i32 as u32); // 06036aec add #4,r0
                r2 = 0x060fff2c; // 06036aee mov.l 0x6036bd4,r2
                cpu.write32(w.wrapping_add(56), r0); // 06036af0 mov.l r0,@(56,gbr)
                cpu.write32(r11.wrapping_add(8), r1); // 06036af2 mov.l r1,@(8,r11)
                cpu.write32(r11.wrapping_add(20), r2); // 06036af4 mov.l r2,@(20,r11)
                // 06036af6 bra 0x6036ace
                r11 = r11.wrapping_add(24i32 as u32); // 06036af8 add #24,r11
                pc = 0x06036ace;
            }
            0x06036afa => {
                r6 = 0i32 as u32; // 06036afa mov #0,r6
                r7 = 0i32 as u32; // 06036afc mov #0,r7
                pc = 0x06036afe;
            }
            0x06036afe => {
                r0 = cpu.read16(r8.wrapping_add(30)) as i16 as i32 as u32; // 06036afe mov.w @(30,r8),r0
                r2 = cpu.read32(r8.wrapping_add(32)); // 06036b00 mov.l @(32,r8),r2
                r0 = r0 & 0xff; // 06036b02 extu.b r0,r0
                r0 <<= 2; // 06036b04 shll2 r0
                t = r6 >= r0; // 06036b06 cmp/hs r0,r6
                let take = t; // 06036b08 bt/s 0x6036b0e
                r1 = cpu.read32(r0.wrapping_add(r12)); // 06036b0a mov.l @(r0,r12),r1
                pc = if take { 0x06036b0e } else { 0x06036b0c };
            }
            0x06036b0c => {
                r6 = r0; // 06036b0c mov r0,r6
                pc = 0x06036b0e;
            }
            0x06036b0e => {
                cpu.write32(r8.wrapping_add(32), r1); // 06036b0e mov.l r1,@(32,r8)
                r7 = r7.wrapping_add(1i32 as u32); // 06036b10 add #1,r7
                cpu.write32(r0.wrapping_add(r12), r8); // 06036b12 mov.l r8,@(r0,r12)
                t = r2 & r2 == 0; // 06036b14 tst r2,r2
                let take = !t; // 06036b16 bf/s 0x6036afe
                r8 = r2; // 06036b18 mov r2,r8
                pc = if take { 0x06036afe } else { 0x06036b1a };
            }
            0x06036b1a => {
                cpu.write32(r10.wrapping_add(4), r2); // 06036b1a mov.l r2,@(4,r10)
                r8 = r12; // 06036b1c mov r12,r8
                r8 = r8.wrapping_add(r6); // 06036b1e add r6,r8
                pc = 0x06036b20;
            }
            0x06036b20 => {
                r6 = cpu.read32(r8); // 06036b20 mov.l @r8,r6
                r8 = r8.wrapping_add(-4i32 as u32); // 06036b22 add #-4,r8
                t = r6 & r6 == 0; // 06036b24 tst r6,r6
                pc = if t { 0x06036b20 } else { 0x06036b28 }; // 06036b26 bt 0x6036b20
            }
            0x06036b28 => {
                r0 = cpu.read32(r6.wrapping_add(32)); // 06036b28 mov.l @(32,r6),r0
                cpu.write32(r11.wrapping_add(8), r6); // 06036b2a mov.l r6,@(8,r11)
                r11 = r11.wrapping_add(12i32 as u32); // 06036b2c add #12,r11
                r6 = r0; // 06036b2e mov r0,r6
                t = r0 & r0 == 0; // 06036b30 tst r0,r0
                let take = !t; // 06036b32 bf/s 0x6036b28
                r7 = r7.wrapping_sub(1); t = r7 == 0; // 06036b34 dt r7
                pc = if take { 0x06036b28 } else { 0x06036b36 };
            }
            0x06036b36 => {
                let take = !t; // 06036b36 bf/s 0x6036b20
                cpu.write32(r8.wrapping_add(4), r0); // 06036b38 mov.l r0,@(4,r8)
                pc = if take { 0x06036b20 } else { 0x06036b3a };
            }
            0x06036b3a => {
                t = r9 & r9 == 0; // 06036b3a tst r9,r9
                let take = !t; // 06036b3c bf/s 0x6036ad2
                r8 = cpu.read32(r10); // 06036b3e mov.l @r10,r8
                pc = if take { 0x06036ad2 } else { 0x06036b40 };
            }
            0x06036b40 => {
                r0 = cpu.read8(w.wrapping_add(169)) as i8 as i32 as u32; // 06036b40 mov.b @(169,gbr),r0
                t = r0 & 32 == 0; // 06036b42 tst #32,r0
                let take = t; // 06036b44 bt/s 0x6036b5c
                r0 = 0i32 as u32; // 06036b46 mov #0,r0
                pc = if take { 0x06036b5c } else { 0x06036b48 };
            }
            0x06036b48 => {
                cpu.write8(w.wrapping_add(169), r0 as u8); // 06036b48 mov.b r0,@(169,gbr)
                r0 = r10; // 06036b4a mov r10,r0
                r0 = r0.wrapping_add(4i32 as u32); // 06036b4c add #4,r0
                r1 = 0x060fff00; // 06036b4e mov.l 0x6036bcc,r1
                r2 = 0x060fff2c; // 06036b50 mov.l 0x6036bd4,r2
                cpu.write32(w.wrapping_add(56), r0); // 06036b52 mov.l r0,@(56,gbr)
                cpu.write32(r11.wrapping_add(8), r1); // 06036b54 mov.l r1,@(8,r11)
                cpu.write32(r11.wrapping_add(20), r2); // 06036b56 mov.l r2,@(20,r11)
                // 06036b58 bra 0x6036ace
                r11 = r11.wrapping_add(24i32 as u32); // 06036b5a add #24,r11
                pc = 0x06036ace;
            }
            0x06036b5c => {
                r2 = 0x86036bbe; // 06036b5c mov.l 0x6036bd8,r2
                cpu.write32(r11.wrapping_add(8), r2); // 06036b5e mov.l r2,@(8,r11)
                r1 = cpu.read32(r11.wrapping_add(4)); // 06036b60 mov.l @(4,r11),r1
                r0 = 1i32 as u32; // 06036b62 mov #1,r0
                cpu.write8(w.wrapping_add(168), r0 as u8); // 06036b64 mov.b r0,@(168,gbr)
                r0 = r1 & 0xffff; // 06036b66 extu.w r1,r0
                r0 >>= 2; // 06036b68 shlr2 r0
                r0 >>= 2; // 06036b6a shlr2 r0
                t = r0 & 1 != 0; r0 >>= 1; // 06036b6c shlr r0
                r0 = r0.wrapping_add(-3i32 as u32); // 06036b6e add #-3,r0
                cpu.write16(w.wrapping_add(164), r0 as u16); // 06036b70 mov.w r0,@(164,gbr)
                r0 = -1i32 as u32; // 06036b72 mov #-1,r0
                cpu.write8(w.wrapping_add(166), r0 as u8); // 06036b74 mov.b r0,@(166,gbr)
                r6 = 0x060ffefc; // 06036b76 mov.l 0x6036bc8,r6
                r4 = r4.wrapping_add(-4i32 as u32); // 06036b78 add #-4,r4
                r7 = 0x25c00020; // 06036b7a mov.l 0x6036bec,r7
                r0 = cpu.read32(r6.wrapping_add(0)); // 06036b7c mov.l @(0,r6),r0
                r1 = cpu.read32(r6.wrapping_add(12)); // 06036b7e mov.l @(12,r6),r1
                r2 = cpu.read32(r6.wrapping_add(20)); // 06036b80 mov.l @(20,r6),r2
                cpu.write32(r7.wrapping_add(0), r0); // 06036b82 mov.l r0,@(0,r7)
                cpu.write32(r7.wrapping_add(12), r1); // 06036b84 mov.l r1,@(12,r7)
                cpu.write32(r7.wrapping_add(20), r2); // 06036b86 mov.l r2,@(20,r7)
                r0 = cpu.read32(r6.wrapping_add(44)); // 06036b88 mov.l @(44,r6),r0
                r1 = cpu.read32(r6.wrapping_add(56)); // 06036b8a mov.l @(56,r6),r1
                cpu.write32(r7.wrapping_add(32), r0); // 06036b8c mov.l r0,@(32,r7)
                cpu.write32(r7.wrapping_add(44), r1); // 06036b8e mov.l r1,@(44,r7)
                r2 = 0x060089c4; // 06036b90 mov.l 0x6036bdc,r2
                r0 = cpu.read32(w.wrapping_add(64)); // 06036b92 mov.l @(64,gbr),r0
                r2 = cpu.read32(r2); // 06036b94 mov.l @r2,r2
                r0 = r0.wrapping_add(r2); // 06036b96 add r2,r0
                cpu.write32(w.wrapping_add(52), r0); // 06036b98 mov.l r0,@(52,gbr)
                r1 = -92i32 as u32; // 06036b9a mov #-92,r1
                r1 <<= 2; // 06036b9c shll2 r1
                r0 = 17i32 as u32; // 06036b9e mov #17,r0
                cpu.write8(r1.wrapping_add(2), r0 as u8); // 06036ba0 mov.b r0,@(2,r1)
                // nop // 06036ba2 nop 
                // nop // 06036ba4 nop 
                // nop // 06036ba6 nop 
                // nop // 06036ba8 nop 
                // nop // 06036baa nop 
                // nop // 06036bac nop 
                // nop // 06036bae nop 
                // nop // 06036bb0 nop 
                // nop // 06036bb2 nop 
                // nop // 06036bb4 nop 
                // 06036bb6 rts
                // nop // 06036bb8 nop 
                if pr == entry_pr { break; }
                pc = pr;
            }
            _ => unreachable!("{pc:#010x}"),
        }
    }
    hand_over!();
    cpu.leave_r14(r14);
}


/// 8: the slave's screen set up for a TV mode (the first word): the screen's width and height from
/// SGL's table of modes (the windows' size, the default depth the width), both windows back to
/// level 1, far limit 0x3fff and that depth, the light, the points room and polygon buffers, the
/// depth tables cleared (0x060361ec, 64 pairs a call), the shade tables made (0x060361fe), VDP1's
/// command list laid out (a jump every 32 bytes), the cache purged. SGL's code block by block
/// (asm2sm.py).
pub fn slave_screen(cpu: &mut Cpu) {
    slave_handler_8_blocks(cpu);
}

#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
fn slave_handler_8_blocks(cpu: &mut Cpu) {
    let w = cpu.gbr();
    let [mut r0, mut r1, mut r2, mut r3, mut r4, mut r5, mut r6, mut r7, mut r8, mut r9, mut r10, mut r11, mut r12, mut r13, _, mut r15] = cpu.sh2.r;
    let mut r14 = cpu.caller_r14;
    let mut mac = ((cpu.sh2.mach as u64) << 32 | cpu.sh2.macl as u64) as i64;
    let mut t = cpu.sh2.sr & 1 != 0;
    let entry_pr = cpu.sh2.pr;
    let mut pr = entry_pr;
    macro_rules! hand_over { () => { cpu.sh2.r = [r0, r1, r2, r3, r4, r5, r6, r7, r8, r9, r10, r11, r12, r13, r14, r15]; cpu.leave_mac(mac); cpu.leave_t(t); } }
    macro_rules! take_back { () => { [r0, r1, r2, r3, r4, r5, r6, r7, r8, r9, r10, r11, r12, r13, r14, r15] = cpu.sh2.r; mac = ((cpu.sh2.mach as u64) << 32 | cpu.sh2.macl as u64) as i64; t = cpu.sh2.sr & 1 != 0; } }
    let mut pc: u32 = 0x06036292;
    loop {
        match pc {
            0x06036292 => {
                r2 = 0x060357a0; // 06036292 mov.l 0x603637c,r2
                r0 &= 124; // 06036294 and #124,r0
                r0 = r0.wrapping_add(r2); // 06036296 add r2,r0
                r2 = cpu.read16(r0) as i16 as i32 as u32; r0 = r0.wrapping_add(2); // 06036298 mov.w @r0+,r2
                r3 = cpu.read16(r0) as i16 as i32 as u32; // 0603629a mov.w @r0,r3
                r5 = 0x060fff28; // 0603629c mov.l 0x6036380,r5
                r7 = 0x060364cc; // 0603629e mov.l 0x6036390,r7
                r0 = r3.rotate_left(16); // 060362a0 swap.w r3,r0
                r0 = (r2 << 16) | (r0 >> 16); // 060362a2 xtrct r2,r0
                cpu.write32(r5.wrapping_add(12), r0); // 060362a4 mov.l r0,@(12,r5)
                cpu.write32(r5.wrapping_add(16), r0); // 060362a6 mov.l r0,@(16,r5)
                cpu.write32(w.wrapping_add(148), r0); // 060362a8 mov.l r0,@(148,gbr)
                r0 = r0.wrapping_add(r0); // 060362aa add r0,r0
                cpu.write32(w.wrapping_add(816), r0); // 060362ac mov.l r0,@(816,gbr)
                r0 = r2.wrapping_neg(); // 060362ae neg r2,r0
                r1 = r3.wrapping_neg(); // 060362b0 neg r3,r1
                r1 <<= 16; // 060362b2 shll16 r1
                r1 = (r0 << 16) | (r1 >> 16); // 060362b4 xtrct r0,r1
                cpu.write32(r7.wrapping_add(12), r1); // 060362b6 mov.l r1,@(12,r7)
                r0 = r2.rotate_left(16); // 060362b8 swap.w r2,r0
                cpu.write32(w.wrapping_add(108), r0); // 060362ba mov.l r0,@(108,gbr)
                r0 = 10i32 as u32; // 060362bc mov #10,r0
                cpu.write16(r5.wrapping_add(4), r0 as u16); // 060362be mov.w r0,@(4,r5)
                r0 = 0x100a000c; // 060362c0 mov.l 0x6036394,r0
                cpu.write32(r5.wrapping_add(0), r0); // 060362c2 mov.l r0,@(0,r5)
                r5 = 0x00000140; // 060362c4 mov.w 0x6036376,r5
                r0 = 0x00000200; // 060362c6 mov.w 0x6036378,r0
                r5 = r5.wrapping_add(r2); // 060362c8 add r2,r5
                r0 = r0.wrapping_add(r3); // 060362ca add r3,r0
                r0 <<= 16; // 060362cc shll16 r0
                r0 = (r5 << 16) | (r0 >> 16); // 060362ce xtrct r5,r0
                cpu.write32(w.wrapping_add(156), r0); // 060362d0 mov.l r0,@(156,gbr)
                r5 = 0x060ffefc; // 060362d2 mov.l 0x6036488,r5
                r2 = r2.wrapping_add(r2); // 060362d4 add r2,r2
                r3 = r3.wrapping_add(r3); // 060362d6 add r3,r3
                r2 = r2.wrapping_add(-1i32 as u32); // 060362d8 add #-1,r2
                r3 = r3.wrapping_add(-1i32 as u32); // 060362da add #-1,r3
                r3 <<= 16; // 060362dc shll16 r3
                r3 = (r2 << 16) | (r3 >> 16); // 060362de xtrct r2,r3
                cpu.write32(r5.wrapping_add(20), r3); // 060362e0 mov.l r3,@(20,r5)
                cpu.write32(r5.wrapping_add(24), r3); // 060362e2 mov.l r3,@(24,r5)
                cpu.write32(r7.wrapping_add(16), r3); // 060362e4 mov.l r3,@(16,r7)
                r0 = 1i32 as u32; // 060362e6 mov #1,r0
                cpu.write8(w.wrapping_add(173), r0 as u8); // 060362e8 mov.b r0,@(173,gbr)
                r1 = r5; // 060362ea mov r5,r1
                r1 = r1.wrapping_add(31i32 as u32); // 060362ec add #31,r1
                cpu.write8(r1.wrapping_add(0), r0 as u8); // 060362ee mov.b r0,@(0,r1)
                cpu.write8(r1.wrapping_add(4), r0 as u8); // 060362f0 mov.b r0,@(4,r1)
                r0 = 8i32 as u32; // 060362f2 mov #8,r0
                cpu.write16(r5.wrapping_add(4), r0 as u16); // 060362f4 mov.w r0,@(4,r5)
                r0 = 0x10080008; // 060362f6 mov.l 0x6036398,r0
                cpu.write32(r5.wrapping_add(0), r0); // 060362f8 mov.l r0,@(0,r5)
                r0 = 0x00003fff; // 060362fa mov.w 0x6036374,r0
                r1 = r5; // 060362fc mov r5,r1
                cpu.write16(w.wrapping_add(160), r0 as u16); // 060362fe mov.w r0,@(160,gbr)
                r1 = r1.wrapping_add(28i32 as u32); // 06036300 add #28,r1
                cpu.write16(r1, r0 as u16); // 06036302 mov.w r0,@r1
                cpu.write16(r1.wrapping_add(4), r0 as u16); // 06036304 mov.w r0,@(4,r1)
                r0 = cpu.read32(w.wrapping_add(108)); // 06036306 mov.l @(108,gbr),r0
                cpu.write32(r5.wrapping_add(36), r0); // 06036308 mov.l r0,@(36,r5)
                cpu.write32(r5.wrapping_add(40), r0); // 0603630a mov.l r0,@(40,r5)
                r0 = 0i32 as u32; // 0603630c mov #0,r0
                cpu.write32(r5.wrapping_add(12), r0); // 0603630e mov.l r0,@(12,r5)
                cpu.write32(r5.wrapping_add(16), r0); // 06036310 mov.l r0,@(16,r5)
                cpu.write8(w.wrapping_add(169), r0 as u8); // 06036312 mov.b r0,@(169,gbr)
                cpu.write32(w.wrapping_add(900), r0); // 06036314 mov.l r0,@(900,gbr)
                cpu.write32(w.wrapping_add(908), r0); // 06036316 mov.l r0,@(908,gbr)
                r0 = 1i32 as u32; // 06036318 mov #1,r0
                cpu.write32(w.wrapping_add(892), r0); // 0603631a mov.l r0,@(892,gbr)
                r0 = 0x060089c4; // 0603631c mov.l 0x60364c8,r0
                r2 = 0x060089c8; // 0603631e mov.l 0x603648c,r2
                r0 = cpu.read32(r0); // 06036320 mov.l @r0,r0
                cpu.write32(w.wrapping_add(52), r0); // 06036322 mov.l r0,@(52,gbr)
                r0 = cpu.read32(r2); // 06036324 mov.l @r2,r0
                cpu.write32(w.wrapping_add(40), r0); // 06036326 mov.l r0,@(40,gbr)
                r0 = 0i32 as u32; // 06036328 mov #0,r0
                cpu.write32(w.wrapping_add(88), r0); // 0603632a mov.l r0,@(88,gbr)
                cpu.write32(w.wrapping_add(92), r0); // 0603632c mov.l r0,@(92,gbr)
                cpu.write32(w.wrapping_add(64), r0); // 0603632e mov.l r0,@(64,gbr)
                r0 = -1i32 as u32; // 06036330 mov #-1,r0
                r0 <<= 16; // 06036332 shll16 r0
                cpu.write32(w.wrapping_add(96), r0); // 06036334 mov.l r0,@(96,gbr)
                r0 = 0x060089c0; // 06036336 mov.l 0x60364c0,r0
                r15 = r15.wrapping_sub(4); cpu.write32(r15, pr); // 06036338 sts.l pr,@-r15
                r0 = cpu.read32(r0); // 0603633a mov.l @r0,r0
                let to = 0x060361ec; // 0603633c bsr 0x60361ec
                r2 = r0; // 0603633e mov r0,r2
                hand_over!(); cpu.call(to, &[]); take_back!();
                pr = 0x06036340;
                pc = 0x06036340;
            }
            0x06036340 => {
                r0 = r2; // 06036340 mov r2,r0
                let to = 0x060361ec; // 06036342 bsr 0x60361ec
                cpu.write32(w.wrapping_add(56), r0); // 06036344 mov.l r0,@(56,gbr)
                hand_over!(); cpu.call(to, &[]); take_back!();
                pr = 0x06036346;
                pc = 0x06036346;
            }
            0x06036346 => {
                let to = 0x060361ec; // 06036346 bsr 0x60361ec
                // nop // 06036348 nop 
                hand_over!(); cpu.call(to, &[]); take_back!();
                pr = 0x0603634a;
                pc = 0x0603634a;
            }
            0x0603634a => {
                let to = 0x060361ec; // 0603634a bsr 0x60361ec
                // nop // 0603634c nop 
                hand_over!(); cpu.call(to, &[]); take_back!();
                pr = 0x0603634e;
                pc = 0x0603634e;
            }
            0x0603634e => {
                r0 = 0x060089cc; // 0603634e mov.l 0x6036384,r0
                r0 = cpu.read32(r0); // 06036350 mov.l @r0,r0
                let to = 0x060361fe; // 06036352 bsr 0x60361fe
                cpu.write32(w.wrapping_add(100), r0); // 06036354 mov.l r0,@(100,gbr)
                hand_over!(); cpu.call(to, &[]); take_back!();
                pr = 0x06036356;
                pc = 0x06036356;
            }
            0x06036356 => {
                r3 = 0x060089bc; // 06036356 mov.l 0x60364c4,r3
                r1 = 0x06004064; // 06036358 mov.l 0x603638c,r1
                r3 = cpu.read32(r3); // 0603635a mov.l @r3,r3
                r1 = cpu.read32(r1); // 0603635c mov.l @r1,r1
                r0 = 0x25c00060; // 0603635e mov.l 0x6036490,r0
                r1 = r1.wrapping_add(r3); // 06036360 add r3,r1
                r2 = 30i32 as u32; // 06036362 mov #30,r2
                pc = 0x06036364;
            }
            0x06036364 => {
                cpu.write32(r3.wrapping_add(4), r0); // 06036364 mov.l r0,@(4,r3)
                cpu.write32(r3.wrapping_add(0), r2); // 06036366 mov.l r2,@(0,r3)
                r3 = r3.wrapping_add(12i32 as u32); // 06036368 add #12,r3
                t = r3 as i32 >= r1 as i32; // 0603636a cmp/ge r1,r3
                let take = !t; // 0603636c bf/s 0x6036364
                r0 = r0.wrapping_add(32i32 as u32); // 0603636e add #32,r0
                pc = if take { 0x06036364 } else { 0x06036370 };
            }
            0x06036370 => {
                // 06036370 bra 0x60364a0
                pr = cpu.read32(r15); r15 = r15.wrapping_add(4); // 06036372 lds.l @r15+,pr
                pc = 0x060364a0;
            }
            0x060364a0 => {
                r1 = -92i32 as u32; // 060364a0 mov #-92,r1
                r1 <<= 2; // 060364a2 shll2 r1
                r0 = 17i32 as u32; // 060364a4 mov #17,r0
                cpu.write8(r1.wrapping_add(2), r0 as u8); // 060364a6 mov.b r0,@(2,r1)
                // nop // 060364a8 nop 
                // nop // 060364aa nop 
                // nop // 060364ac nop 
                // nop // 060364ae nop 
                // nop // 060364b0 nop 
                // nop // 060364b2 nop 
                // nop // 060364b4 nop 
                // nop // 060364b6 nop 
                // nop // 060364b8 nop 
                // nop // 060364ba nop 
                // 060364bc rts
                // nop // 060364be nop 
                if pr == entry_pr { break; }
                pc = pr;
            }
            _ => unreachable!("{pc:#010x}"),
        }
    }
    hand_over!();
    cpu.leave_r14(r14);
}


/// 0x060361ec (slave_screen's): 64 pairs of longs from r2 on cleared.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn slave_clear_pairs(cpu: &mut Cpu) {
    let w = cpu.gbr();
    let [mut r0, mut r1, mut r2, mut r3, mut r4, mut r5, mut r6, mut r7, mut r8, mut r9, mut r10, mut r11, mut r12, mut r13, _, mut r15] = cpu.sh2.r;
    let mut r14 = cpu.caller_r14;
    let mut mac = ((cpu.sh2.mach as u64) << 32 | cpu.sh2.macl as u64) as i64;
    let mut t = cpu.sh2.sr & 1 != 0;
    let entry_pr = cpu.sh2.pr;
    let mut pr = entry_pr;
    macro_rules! hand_over { () => { cpu.sh2.r = [r0, r1, r2, r3, r4, r5, r6, r7, r8, r9, r10, r11, r12, r13, r14, r15]; cpu.leave_mac(mac); cpu.leave_t(t); } }
    macro_rules! take_back { () => { [r0, r1, r2, r3, r4, r5, r6, r7, r8, r9, r10, r11, r12, r13, r14, r15] = cpu.sh2.r; mac = ((cpu.sh2.mach as u64) << 32 | cpu.sh2.macl as u64) as i64; t = cpu.sh2.sr & 1 != 0; } }
    let mut pc: u32 = 0x060361ec;
    loop {
        match pc {
            0x060361ec => {
                r0 = 0i32 as u32; // 060361ec mov #0,r0
                r1 = 64i32 as u32; // 060361ee mov #64,r1
                pc = 0x060361f0;
            }
            0x060361f0 => {
                cpu.write32(r2, r0); // 060361f0 mov.l r0,@r2
                cpu.write32(r2.wrapping_add(4), r0); // 060361f2 mov.l r0,@(4,r2)
                r1 = r1.wrapping_sub(1); t = r1 == 0; // 060361f4 dt r1
                let take = !t; // 060361f6 bf/s 0x60361f0
                r2 = r2.wrapping_add(8i32 as u32); // 060361f8 add #8,r2
                pc = if take { 0x060361f0 } else { 0x060361fa };
            }
            0x060361fa => {
                // 060361fa rts
                // nop // 060361fc nop 
                if pr == entry_pr { break; }
                pc = pr;
            }
            _ => unreachable!("{pc:#010x}"),
        }
    }
    hand_over!();
    cpu.leave_r14(r14);
}


/// 0x060361fe (slave_screen's): SGL's shade tables made at r0: 32 rows of three bytes (the
/// shades' brightness ramp, 0-31 clamped, for each of three planes 0x400 apart), then the grey
/// Gouraud entries at the end of VDP1's VRAM (32 of four words, 0x421 apart).
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn slave_shade_tables(cpu: &mut Cpu) {
    let w = cpu.gbr();
    let [mut r0, mut r1, mut r2, mut r3, mut r4, mut r5, mut r6, mut r7, mut r8, mut r9, mut r10, mut r11, mut r12, mut r13, _, mut r15] = cpu.sh2.r;
    let mut r14 = cpu.caller_r14;
    let mut mac = ((cpu.sh2.mach as u64) << 32 | cpu.sh2.macl as u64) as i64;
    let mut t = cpu.sh2.sr & 1 != 0;
    let entry_pr = cpu.sh2.pr;
    let mut pr = entry_pr;
    macro_rules! hand_over { () => { cpu.sh2.r = [r0, r1, r2, r3, r4, r5, r6, r7, r8, r9, r10, r11, r12, r13, r14, r15]; cpu.leave_mac(mac); cpu.leave_t(t); } }
    macro_rules! take_back { () => { [r0, r1, r2, r3, r4, r5, r6, r7, r8, r9, r10, r11, r12, r13, r14, r15] = cpu.sh2.r; mac = ((cpu.sh2.mach as u64) << 32 | cpu.sh2.macl as u64) as i64; t = cpu.sh2.sr & 1 != 0; } }
    let mut pc: u32 = 0x060361fe;
    loop {
        match pc {
            0x060361fe => {
                r5 = r0; // 060361fe mov r0,r5
                r2 = 4i32 as u32; // 06036200 mov #4,r2
                r2 <<= 8; // 06036202 shll8 r2
                r3 = 32i32 as u32; // 06036204 mov #32,r3
                r7 = 31i32 as u32; // 06036206 mov #31,r7
                r0 = 16i32 as u32; // 06036208 mov #16,r0
                pc = 0x0603620a;
            }
            0x0603620a => {
                r0 = r0.wrapping_sub(r3); // 0603620a sub r3,r0
                r1 = 32i32 as u32; // 0603620c mov #32,r1
                pc = 0x0603620e;
            }
            0x0603620e => {
                r6 = 0i32 as u32; // 0603620e mov #0,r6
                t = r0 as i32 > 0; // 06036210 cmp/pl r0
                pc = if !t { 0x0603621c } else { 0x06036214 }; // 06036212 bf 0x603621c
            }
            0x06036214 => {
                r6 = r0; // 06036214 mov r0,r6
                t = r6 as i32 > r7 as i32; // 06036216 cmp/gt r7,r6
                pc = if !t { 0x0603621c } else { 0x0603621a }; // 06036218 bf 0x603621c
            }
            0x0603621a => {
                r6 = r7; // 0603621a mov r7,r6
                pc = 0x0603621c;
            }
            0x0603621c => {
                cpu.write8(r5, r6 as u8); // 0603621c mov.b r6,@r5
                r5 = r5.wrapping_add(r2); // 0603621e add r2,r5
                cpu.write8(r5, r6 as u8); // 06036220 mov.b r6,@r5
                r5 = r5.wrapping_add(r2); // 06036222 add r2,r5
                cpu.write8(r5, r6 as u8); // 06036224 mov.b r6,@r5
                r5 = r5.wrapping_sub(r2); // 06036226 sub r2,r5
                r5 = r5.wrapping_sub(r2); // 06036228 sub r2,r5
                r0 = r0.wrapping_add(1i32 as u32); // 0603622a add #1,r0
                r1 = r1.wrapping_sub(1); t = r1 == 0; // 0603622c dt r1
                let take = !t; // 0603622e bf/s 0x603620e
                r5 = r5.wrapping_add(1i32 as u32); // 06036230 add #1,r5
                pc = if take { 0x0603620e } else { 0x06036232 };
            }
            0x06036232 => {
                r3 = r3.wrapping_sub(1); t = r3 == 0; // 06036232 dt r3
                let take = !t; // 06036234 bf/s 0x603620a
                r0 = 16i32 as u32; // 06036236 mov #16,r0
                pc = if take { 0x0603620a } else { 0x06036238 };
            }
            0x06036238 => {
                r3 = 0x25c7ff00; // 06036238 mov.l 0x6036388,r3
                r0 = 0i32 as u32; // 0603623a mov #0,r0
                r2 = 0x00000421; // 0603623c mov.w 0x603639c,r2
                r1 = 32i32 as u32; // 0603623e mov #32,r1
                pc = 0x06036240;
            }
            0x06036240 => {
                cpu.write16(r3.wrapping_add(0), r0 as u16); // 06036240 mov.w r0,@(0,r3)
                cpu.write16(r3.wrapping_add(2), r0 as u16); // 06036242 mov.w r0,@(2,r3)
                cpu.write16(r3.wrapping_add(4), r0 as u16); // 06036244 mov.w r0,@(4,r3)
                cpu.write16(r3.wrapping_add(6), r0 as u16); // 06036246 mov.w r0,@(6,r3)
                r0 = r0.wrapping_add(r2); // 06036248 add r2,r0
                r1 = r1.wrapping_sub(1); t = r1 == 0; // 0603624a dt r1
                let take = !t; // 0603624c bf/s 0x6036240
                r3 = r3.wrapping_add(8i32 as u32); // 0603624e add #8,r3
                pc = if take { 0x06036240 } else { 0x06036250 };
            }
            0x06036250 => {
                // 06036250 rts
                // nop // 06036252 nop 
                if pr == entry_pr { break; }
                pc = pr;
            }
            _ => unreachable!("{pc:#010x}"),
        }
    }
    hand_over!();
    cpu.leave_r14(r14);
}


/// SGL's drawing set-up (slInitSystem's, with the TV mode and the texture table): the command
/// buffer begun with the slave's screen set-up (command 8 with the mode, the slave signalled),
/// VDP1's registers (its TV mode by the screen mode's hi-res bit, the frame change, the plot
/// trigger, the erase colour and area) and the start of its command list (a system clip, the
/// end), the master's window (depth, edges, centre, screen size) from SGL's table of modes, the
/// texture table, the polygon buffer and points room, the counts and light cleared. SGL's code
/// block by block (asm2sm.py).
pub fn draw_init(cpu: &mut Cpu) {
    draw_init_blocks(cpu);
}

#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
fn draw_init_blocks(cpu: &mut Cpu) {
    let w = cpu.gbr();
    let [mut r0, mut r1, mut r2, mut r3, mut r4, mut r5, mut r6, mut r7, mut r8, mut r9, mut r10, mut r11, mut r12, mut r13, _, mut r15] = cpu.sh2.r;
    let mut r14 = cpu.caller_r14;
    let mut mac = ((cpu.sh2.mach as u64) << 32 | cpu.sh2.macl as u64) as i64;
    let mut t = cpu.sh2.sr & 1 != 0;
    let entry_pr = cpu.sh2.pr;
    let mut pr = entry_pr;
    macro_rules! hand_over { () => { cpu.sh2.r = [r0, r1, r2, r3, r4, r5, r6, r7, r8, r9, r10, r11, r12, r13, r14, r15]; cpu.leave_mac(mac); cpu.leave_t(t); } }
    macro_rules! take_back { () => { [r0, r1, r2, r3, r4, r5, r6, r7, r8, r9, r10, r11, r12, r13, r14, r15] = cpu.sh2.r; mac = ((cpu.sh2.mach as u64) << 32 | cpu.sh2.macl as u64) as i64; t = cpu.sh2.sr & 1 != 0; } }
    let mut pc: u32 = 0x060364e0;
    loop {
        match pc {
            0x060364e0 => {
                r0 = 0x060089d0; // 060364e0 mov.l 0x60365e4,r0
                r1 = 0x20000000; // 060364e2 mov.l 0x60365e8,r1
                r0 = cpu.read32(r0); // 060364e4 mov.l @r0,r0
                r0 |= r1; // 060364e6 or r1,r0
                cpu.write32(w.wrapping_add(68), r0); // 060364e8 mov.l r0,@(68,gbr)
                cpu.write32(w.wrapping_add(72), r0); // 060364ea mov.l r0,@(72,gbr)
                r3 = r0; // 060364ec mov r0,r3
                r7 = 0x25c00000; // 060364ee mov.l 0x60365ec,r7
                r6 = 0x25d00000; // 060364f0 mov.l 0x60365f0,r6
                r0 = 0xffff8000; // 060364f2 mov.w 0x60365da,r0
                cpu.write16(r7, r0 as u16); // 060364f4 mov.w r0,@r7
                r0 = cpu.read8(w.wrapping_add(176)) as i8 as i32 as u32; // 060364f6 mov.b @(176,gbr),r0
                r1 = 31i32 as u32; // 060364f8 mov #31,r1
                r4 &= r1; // 060364fa and r1,r4
                r0 &= 128; // 060364fc and #128,r0
                r0 |= r4; // 060364fe or r4,r0
                t = r0 & 16 == 0; // 06036500 tst #16,r0
                pc = if t { 0x06036506 } else { 0x06036504 }; // 06036502 bt 0x6036506
            }
            0x06036504 => {
                r0 &= 127; // 06036504 and #127,r0
                pc = 0x06036506;
            }
            0x06036506 => {
                cpu.write8(w.wrapping_add(176), r0 as u8); // 06036506 mov.b r0,@(176,gbr)
                r0 >>= 2; // 06036508 shlr2 r0
                t = r0 & 1 != 0; r0 >>= 1; // 0603650a shlr r0
                r0 &= 1; // 0603650c and #1,r0
                r4 <<= 2; // 0603650e shll2 r4
                cpu.write16(r6.wrapping_add(0), r0 as u16); // 06036510 mov.w r0,@(0,r6)
                cpu.write32(r3.wrapping_add(4), r4); // 06036512 mov.l r4,@(4,r3)
                r1 = 8i32 as u32; // 06036514 mov #8,r1
                cpu.write32(r3.wrapping_add(0), r1); // 06036516 mov.l r1,@(0,r3)
                r0 = r3; // 06036518 mov r3,r0
                r1 = 0x21000000; // 0603651a mov.l 0x603660c,r1
                r0 = r0.wrapping_add(8i32 as u32); // 0603651c add #8,r0
                cpu.write32(w.wrapping_add(72), r0); // 0603651e mov.l r0,@(72,gbr)
                cpu.write16(r1, r0 as u16); // 06036520 mov.w r0,@r1
                r0 = r5; // 06036522 mov r5,r0
                cpu.write32(w.wrapping_add(60), r0); // 06036524 mov.l r0,@(60,gbr)
                r0 = 0x060357a0; // 06036526 mov.l 0x6036608,r0
                r1 = 1i32 as u32; // 06036528 mov #1,r1
                r2 = cpu.read32(r0.wrapping_add(r4)); // 0603652a mov.l @(r0,r4),r2
                r1 <<= 16; // 0603652c shll16 r1
                r0 = r2.rotate_left(16); // 0603652e swap.w r2,r0
                r0 <<= 16; // 06036530 shll16 r0
                cpu.write32(w.wrapping_add(104), r0); // 06036532 mov.l r0,@(104,gbr)
                r0 = r2.wrapping_neg(); // 06036534 neg r2,r0
                r0 = r0.wrapping_add(r1); // 06036536 add r1,r0
                cpu.write32(w.wrapping_add(120), r0); // 06036538 mov.l r0,@(120,gbr)
                r0 = 0x01400200; // 0603653a mov.l 0x60365e0,r0
                r1 = r1.wrapping_add(1i32 as u32); // 0603653c add #1,r1
                r0 = r0.wrapping_add(r2); // 0603653e add r2,r0
                cpu.write32(w.wrapping_add(152), r0); // 06036540 mov.l r0,@(152,gbr)
                r0 = r2; // 06036542 mov r2,r0
                cpu.write32(w.wrapping_add(144), r0); // 06036544 mov.l r0,@(144,gbr)
                r0 = r0.wrapping_sub(r1); // 06036546 sub r1,r0
                cpu.write32(w.wrapping_add(124), r0); // 06036548 mov.l r0,@(124,gbr)
                r2 = r2.wrapping_add(r2); // 0603654a add r2,r2
                r0 = r2; // 0603654c mov r2,r0
                cpu.write32(w.wrapping_add(128), r0); // 0603654e mov.l r0,@(128,gbr)
                cpu.write32(w.wrapping_add(132), r0); // 06036550 mov.l r0,@(132,gbr)
                r0 = r0.wrapping_sub(r1); // 06036552 sub r1,r0
                cpu.write32(r7.wrapping_add(20), r0); // 06036554 mov.l r0,@(20,r7)
                r0 = 0i32 as u32; // 06036556 mov #0,r0
                cpu.write32(w.wrapping_add(896), r0); // 06036558 mov.l r0,@(896,gbr)
                cpu.write32(w.wrapping_add(904), r0); // 0603655a mov.l r0,@(904,gbr)
                r0 = 1i32 as u32; // 0603655c mov #1,r0
                cpu.write32(w.wrapping_add(888), r0); // 0603655e mov.l r0,@(888,gbr)
                r0 = 0x02000140; // 06036560 mov.l 0x60365dc,r0
                cpu.write32(w.wrapping_add(772), r0); // 06036562 mov.l r0,@(772,gbr)
                r3 <<= 16; // 06036564 shll16 r3
                r3 |= r0; // 06036566 or r0,r3
                r3 = r3.wrapping_add(r3); // 06036568 add r3,r3
                r0 = r2; // 0603656a mov r2,r0
                r0 = r0.wrapping_add(r3); // 0603656c add r3,r0
                cpu.write32(w.wrapping_add(140), r0); // 0603656e mov.l r0,@(140,gbr)
                r1 = 0xffff8000; // 06036570 mov.w 0x60365da,r1
                r0 = r7; // 06036572 mov r7,r0
                r0 = r0.wrapping_add(32i32 as u32); // 06036574 add #32,r0
                cpu.write16(r0, r1 as u16); // 06036576 mov.w r1,@r0
                r0 = 0x060089c4; // 06036578 mov.l 0x60365f4,r0
                r1 = 0x06004068; // 0603657a mov.l 0x60365f8,r1
                r0 = cpu.read32(r0); // 0603657c mov.l @r0,r0
                r1 = cpu.read32(r1); // 0603657e mov.l @r1,r1
                r0 = r0.wrapping_add(-36i32 as u32); // 06036580 add #-36,r0
                t = r1 & 1 != 0; r1 >>= 1; // 06036582 shlr r1
                r0 = r0.wrapping_add(r1); // 06036584 add r1,r0
                cpu.write32(w.wrapping_add(48), r0); // 06036586 mov.l r0,@(48,gbr)
                r0 = 0i32 as u32; // 06036588 mov #0,r0
                cpu.write32(w.wrapping_add(64), r0); // 0603658a mov.l r0,@(64,gbr)
                r1 = 0x10090004; // 0603658c mov.l 0x6036610,r1
                cpu.write32(r7, r1); // 0603658e mov.l r1,@r7
                r0 = 0i32 as u32; // 06036590 mov #0,r0
                cpu.write16(r6.wrapping_add(2), r0 as u16); // 06036592 mov.w r0,@(2,r6)
                r0 = 2i32 as u32; // 06036594 mov #2,r0
                cpu.write16(r6.wrapping_add(4), r0 as u16); // 06036596 mov.w r0,@(4,r6)
                r0 = 0i32 as u32; // 06036598 mov #0,r0
                cpu.write16(w.wrapping_add(114), r0 as u16); // 0603659a mov.w r0,@(114,gbr)
                cpu.write16(r6.wrapping_add(6), r0 as u16); // 0603659c mov.w r0,@(6,r6)
                cpu.write16(r6.wrapping_add(8), r0 as u16); // 0603659e mov.w r0,@(8,r6)
                r0 = 0x000058ff; // 060365a0 mov.w 0x60365d6,r0
                r1 = 0x060089c8; // 060365a2 mov.l 0x60365fc,r1
                r2 = 0x06004058; // 060365a4 mov.l 0x6036604,r2
                r3 = 0x0600405a; // 060365a6 mov.l 0x6036600,r3
                cpu.write16(r6.wrapping_add(10), r0 as u16); // 060365a8 mov.w r0,@(10,r6)
                r0 = cpu.read32(r1); // 060365aa mov.l @r1,r0
                r2 = cpu.read16(r2) as i16 as i32 as u32; // 060365ac mov.w @r2,r2
                r3 = cpu.read16(r3) as i16 as i32 as u32; // 060365ae mov.w @r3,r3
                cpu.write32(w.wrapping_add(36), r0); // 060365b0 mov.l r0,@(36,gbr)
                r0 = r2.rotate_left(16); // 060365b2 swap.w r2,r0
                r0 = (r3 << 16) | (r0 >> 16); // 060365b4 xtrct r3,r0
                cpu.write32(w.wrapping_add(136), r0); // 060365b6 mov.l r0,@(136,gbr)
                r0 = 0x00003fff; // 060365b8 mov.w 0x60365d8,r0
                cpu.write16(w.wrapping_add(112), r0 as u16); // 060365ba mov.w r0,@(112,gbr)
                r0 = 0i32 as u32; // 060365bc mov #0,r0
                cpu.write8(w.wrapping_add(168), r0 as u8); // 060365be mov.b r0,@(168,gbr)
                cpu.write32(w.wrapping_add(116), r0); // 060365c0 mov.l r0,@(116,gbr)
                cpu.write32(w.wrapping_add(76), r0); // 060365c2 mov.l r0,@(76,gbr)
                cpu.write32(w.wrapping_add(80), r0); // 060365c4 mov.l r0,@(80,gbr)
                r0 = -1i32 as u32; // 060365c6 mov #-1,r0
                cpu.write8(w.wrapping_add(166), r0 as u8); // 060365c8 mov.b r0,@(166,gbr)
                r0 <<= 16; // 060365ca shll16 r0
                cpu.write32(w.wrapping_add(84), r0); // 060365cc mov.l r0,@(84,gbr)
                r0 = 1i32 as u32; // 060365ce mov #1,r0
                cpu.write8(w.wrapping_add(166), r0 as u8); // 060365d0 mov.b r0,@(166,gbr)
                // 060365d2 rts
                cpu.write8(w.wrapping_add(172), r0 as u8); // 060365d4 mov.b r0,@(172,gbr)
                if pr == entry_pr { break; }
                pc = pr;
            }
            _ => unreachable!("{pc:#010x}"),
        }
    }
    hand_over!();
    cpu.leave_r14(r14);
}
