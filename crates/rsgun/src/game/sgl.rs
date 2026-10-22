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

/// The current matrix inverted in place: Gauss-Jordan elimination, as SGL's asm does it. The
/// first two pivots are the biggest left in their column (a row swapped up for it), the third is
/// what is left; the swaps are undone on the columns at the end. Each step divides on the divide
/// unit in the SH-2 code's order, and the registers are left as the SH-2 code leaves them.
pub fn sl_inverse(cpu: &mut Cpu) {
    let m = matrix(cpu);
    let at = |row: usize, col: usize| m + (16 * row + 4 * col) as u32;
    let size = |cpu: &mut Cpu, row: usize, col: usize| (cpu.read32(at(row, col)) as i32).unsigned_abs();
    // the first pivot: the biggest of the first column (a tie to the upper row)
    let sizes = [0, 1, 2].map(|row| size(cpu, row, 0));
    let upper = if sizes[0] >= sizes[1] { 0 } else { 1 };
    let first = if sizes[upper] >= sizes[2] { upper } else { 2 };
    // (the swap passes the lower row's third element through r7)
    let r7 = if first == 0 { cpu.sh2.r[7] } else { cpu.read32(at(first, 2)) };
    swap_rows(cpu, m, 0, first);
    inverse_step(cpu, m, 0, [1, 2]);
    // the second pivot: the bigger of rows 1 and 2 in the second column
    let second = if size(cpu, 1, 1) >= size(cpu, 2, 1) { 1 } else { 2 };
    swap_rows(cpu, m, 1, second);
    inverse_step(cpu, m, 1, [0, 2]);
    let (factors, translation) = inverse_step(cpu, m, 2, [0, 1]);
    // the swaps undone, the second's first
    if second == 2 {
        swap_columns(cpu, m, 1, 2);
    }
    if first != 0 {
        swap_columns(cpu, m, 0, first);
    }
    // the registers: the last swap's values (both columns it swapped, the other one first), or
    // what the third step left
    let product = factors[1] as i32 as i64 * translation as i32 as i64;
    let regs = if first != 0 {
        [at(0, first), at(1, first), at(2, first), at(0, 0), at(1, 0), at(2, 0)].map(|a| cpu.read32(a))
    } else if second == 2 {
        [at(0, 2), at(0, 1), at(1, 2), at(1, 1), at(2, 2), at(1, 3)].map(|a| cpu.read32(a))
    } else {
        [0, 1, cpu.read32(at(1, 2)), 0, (product >> 8) as u32, cpu.read32(at(1, 3))]
    };
    let [r0, r1, r2, r3, r4, r5] = regs;
    cpu.leave(&[(0, r0), (1, r1), (2, r2), (3, r3), (4, r4), (5, r5), (6, m), (7, r7)]);
    cpu.leave_mac(product);
    cpu.leave_t(first != 2);
}

/// One step of SGL's inverse: pivot row and column `p`, the `others` reduced by it. Each other
/// column's element in the other rows less its factor times the pivot row's over the pivot, the
/// pivot row's over the pivot and negated (the translation column at 8 fraction bits, the other
/// rows less products of that); the pivot column's factors over the pivot; the pivot its
/// reciprocal. Returns the other rows' factors and the translation's quotient.
fn inverse_step(cpu: &mut Cpu, m: u32, p: usize, others: [usize; 2]) -> ([u32; 2], u32) {
    let at = |row: usize, col: usize| m + (16 * row + 4 * col) as u32;
    let wide = |v: u32| v as i32 as i64;
    let pivot = cpu.read32(at(p, p));
    let factors = others.map(|row| cpu.read32(at(row, p)));
    for col in (0..3).filter(|&col| col != p) {
        let x = cpu.read32(at(p, col));
        for (row, f) in others.into_iter().zip(factors) {
            let q = divide64(cpu, pivot, wide(f) * wide(x));
            let v = cpu.read32(at(row, col)).wrapping_sub(q);
            cpu.write32(at(row, col), v);
        }
        let q = divide64(cpu, pivot, wide(x) << 16);
        cpu.write32(at(p, col), q.wrapping_neg());
    }
    let x = cpu.read32(at(p, 3));
    let translation = divide64(cpu, pivot, wide(x) << 8);
    for (row, f) in others.into_iter().zip(factors) {
        let v = cpu.read32(at(row, 3)).wrapping_sub(((wide(f) * wide(translation)) >> 8) as u32);
        cpu.write32(at(row, 3), v);
    }
    cpu.write32(at(p, 3), translation.wrapping_neg() << 8);
    for (row, f) in others.into_iter().zip(factors) {
        let q = divide64(cpu, pivot, wide(f) << 16);
        cpu.write32(at(row, p), q);
    }
    let reciprocal = divide64(cpu, pivot, 1 << 32);
    cpu.write32(at(p, p), reciprocal);
    (factors, translation)
}

/// `dividend` (64 bits) over `divisor` on the divide unit, its quotient read back.
fn divide64(cpu: &mut Cpu, divisor: u32, dividend: i64) -> u32 {
    cpu.write32(DVSR, divisor);
    cpu.write32(DVDNTH, (dividend >> 32) as u32);
    cpu.write32(DVDNTL, dividend as u32);
    cpu.read32(DVDNTL_AGAIN)
}

/// Rows `a` and `b` of the matrix at `m` swapped (none when they are the same).
fn swap_rows(cpu: &mut Cpu, m: u32, a: usize, b: usize) {
    if a != b {
        for col in 0..4 {
            swap_words(cpu, m + (16 * a + 4 * col) as u32, m + (16 * b + 4 * col) as u32);
        }
    }
}

/// Columns `a` and `b` of the matrix at `m` swapped, in its three rows.
fn swap_columns(cpu: &mut Cpu, m: u32, a: usize, b: usize) {
    for row in 0..3 {
        swap_words(cpu, m + (16 * row + 4 * a) as u32, m + (16 * row + 4 * b) as u32);
    }
}

fn swap_words(cpu: &mut Cpu, a: u32, b: u32) {
    let (x, y) = (cpu.read32(a), cpu.read32(b));
    cpu.write32(a, y);
    cpu.write32(b, x);
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
fn page_shift(cpu: &mut Cpu, pnc: u32, chctl: u32, size_bit: u32) -> u32 {
    let w = cpu.gbr();
    let mut s = 11;
    if cpu.read16(w + IMAGE + pnc) & 0x8000 == 0 {
        s += 1;
    }
    if cpu.read16(w + chctl) as u32 >> size_bit & 1 == 0 {
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
    let s = page_shift(cpu, PNCN0, CHCTLA, 0);
    map_set(cpu, s, MPOFN + 1, false, MPABN0, [a, b, c, d]);
}

pub fn sl_map_nbg1(cpu: &mut Cpu, a: u32, b: u32, c: u32, d: u32) {
    let w = cpu.gbr();
    cpu.write32(w + SCREEN_MAPS + 4, a);
    let s = page_shift(cpu, PNCN1, CHCTLA, 8);
    map_set(cpu, s, MPOFN + 1, true, MPABN1, [a, b, c, d]);
}

pub fn sl_map_nbg2(cpu: &mut Cpu, a: u32, b: u32, c: u32, d: u32) {
    let w = cpu.gbr();
    cpu.write32(w + SCREEN_MAPS + 8, a);
    let s = page_shift(cpu, PNCN2, CHCTLB, 0);
    map_set(cpu, s, MPOFN, false, MPABN2, [a, b, c, d]);
}

/// slMapNbg3 (OPENING.BIN's copy): its character size is CHCTLB's bit 4.
const MPABN3: u32 = 0x10c;

pub fn sl_map_nbg3(cpu: &mut Cpu, a: u32, b: u32, c: u32, d: u32) {
    let w = cpu.gbr();
    cpu.write32(w + SCREEN_MAPS + 12, a);
    let s = page_shift(cpu, PNCN3, CHCTLB, 4);
    map_set(cpu, s, MPOFN, true, MPABN3, [a, b, c, d]);
}

/// slMapRbg0: RBG0's (its parameter A's) sixteen planes all on the map at `a`, its VRAM bank
/// given to the pattern names.
pub fn sl_map_rbg0(cpu: &mut Cpu, a: u32) {
    let w = cpu.gbr();
    cpu.write32(w + RBG0_MAP, a);
    cpu.call(ROTATION_BANK, &[a, BANK_NAMES]);
    let s = page_shift(cpu, PNCR, CHCTLB, 8);
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

/// slPlaneNbg3 (OPENING.BIN's copy).
pub fn sl_plane_nbg3(cpu: &mut Cpu, size: u32) {
    plane_size(cpu, 0x3f, (size << 8) >> 2);
    cpu.leave(&[(4, (size << 8) >> 2)]);
}

/// slScrPosNbg3, and its move by (dx, dy) (OPENING.BIN's copies): NBG3's place (x, y: 16.16) as
/// SGL keeps it, and its integer parts in the VDP2 image (SCXN3, SCYN3).
const NBG3_X: u32 = 0x2f4;
const NBG3_Y: u32 = 0x2f8;
const SCROLL_NBG3: u32 = 0x154;

pub fn sl_scr_pos_nbg3(cpu: &mut Cpu, x: u32, y: u32) {
    let w = cpu.gbr();
    cpu.write32(w + NBG3_X, x);
    cpu.write32(w + NBG3_Y, y);
    let v = (x & 0xffff_0000) | y >> 16;
    cpu.write32(w + SCROLL_NBG3, v);
    cpu.leave(&[(0, v), (4, x >> 16)]);
}

pub fn sl_scr_move_nbg3(cpu: &mut Cpu, dx: u32, dy: u32) {
    let w = cpu.gbr();
    let x = cpu.read32(w + NBG3_X).wrapping_add(dx);
    cpu.write32(w + NBG3_X, x);
    let y = cpu.read32(w + NBG3_Y).wrapping_add(dy);
    cpu.write32(w + NBG3_Y, y);
    let v = (x & 0xffff_0000) | y >> 16;
    cpu.write32(w + SCROLL_NBG3, v);
    cpu.leave(&[(0, v), (1, x.rotate_left(16))]);
}

/// slSpriteCCalcCond, slSpriteCCalcNum (OPENING.BIN's copies): the sprites' colour calculation
/// condition (SPCTL's SPCCCS, bits 12-13) and number (SPCCN, bits 8-10), set in SPCTL's high
/// byte with its bytes swapped (the register's upper half the read's sign).
fn swap_bytes(v: u32) -> u32 {
    (v & 0xffff_0000) | (v & 0xff) << 8 | (v >> 8) & 0xff
}

fn spctl_high(cpu: &mut Cpu, keep: u32, v: u32) -> u32 {
    let w = cpu.gbr();
    let old = cpu.read16(w + SPCTL) as i16 as i32 as u32;
    let new = swap_bytes((swap_bytes(old) & keep) | v);
    cpu.write16(w + SPCTL, new as u16);
    new
}

pub fn sl_sprite_cc_condition(cpu: &mut Cpu, condition: u32) {
    let v = spctl_high(cpu, !0x30, condition << 4);
    cpu.leave(&[(0, v), (1, !0x30), (4, condition << 4)]);
}

pub fn sl_sprite_cc_number(cpu: &mut Cpu, n: u32) {
    let v = spctl_high(cpu, !7, n);
    cpu.leave(&[(0, v), (1, !7)]);
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

/// The TV mode (`mode`, 0-31; SGL's table of TVMD's low byte by it, and of half the screen's width
/// and height, the width the rotation tables' Pz and Cz), unless its hi-res bit (bit 2) differs
/// from the screen mode's: whether it was set. Mode bit 4 clears the screen mode's bit 7.
const TV_MODES: u32 = 0x0603_5820;
const SCREEN_HALVES: u32 = 0x0603_57a0;
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
    let halves = cpu.read32(SCREEN_HALVES + 4 * mode);
    for at in [0, PZ_TO_CZ, ROTATION_TABLE, ROTATION_TABLE + PZ_TO_CZ] {
        cpu.write16(ROTATION_PZ + at, (halves >> 16) as u16);
    }
    cpu.leave(&[(1, halves), (2, ROTATION_PZ + ROTATION_TABLE)]);
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
/// size scaled, the command linked by its depth), then a distorted sprite with its corners from
/// its place (+12, +16) and its width and height turned by the command's cosine and sine (+24,
/// +28; 16.16). VDP1's zoom point (the control word's bits 8-11) says where the place is on the
/// sprite: its top left, its top edge's middle, its left edge's middle or its centre; the right
/// and bottom points are those with the width's or the height's turn reversed and the control's
/// flip bit toggled to match; a zoom point VDP1 has no such place for is made the centre. Each
/// corner must be inside the window (one out: the command's link undone, nothing drawn). Then
/// the slot moves past the command, and unless the sprite is Gouraud shaded (its colour
/// calculation's bit 2: its table goes by the corners) its corners are turned round to start
/// from the first on the screen, or the nearest when none is (SGL's code takes the third
/// corner's order for the second when the second is the nearest), the flip bits toggled to
/// match. The place in the commands left past them (`-> slave`).
const TURN_COS: u32 = 24;
const TURN_SIN: u32 = 28;
const PLACE_X: u32 = 12;
const PLACE_Y: u32 = 16;
const ZOOM_POINT: u32 = 0x0f00;
const ZOOM_CENTRE: u32 = 0x0a00;
const H_FLIP: u32 = 16;
const V_FLIP: u32 = 32;
const GOURAUD_SHADED: u16 = 4;
const CORNERS: u32 = 12;
const SPRITE_COMMAND_BYTES: u32 = 32;

enum ZoomPoint {
    TopLeft,
    TopMiddle,
    LeftMiddle,
    Centre,
}

pub fn slave_turned_sprite(cpu: &mut Cpu) {
    let Some((entry, control, width, height)) = slave_sprite_start(cpu, 28) else {
        return;
    };
    let w = cpu.gbr();
    let base = cpu.reg(4);
    cpu.leave(&[(4, base + SPRITE_COMMAND_BYTES)]);
    let cos = cpu.read32(base + TURN_COS);
    let sin = cpu.read32(base + TURN_SIN);
    let turned = |a: u32, b: u32| ((a as i32 as i64 * b as i32 as i64) >> 16) as u32;
    let (mut wc, mut hc, mut ws, mut hs) = (turned(width, cos), turned(height, cos), turned(width, sin), turned(height, sin));
    let window = cpu.read32(w + WIN_SIZE_SCREEN);
    let (x_limit, y_limit) = (window >> 16 << 16, window << 16);
    let screen = cpu.read32(w + SLAVE_SCREEN);
    let (x_off, y_off) = (screen >> 16 << 16, screen << 16);
    let (x, y) = (cpu.read32(base + PLACE_X), cpu.read32(base + PLACE_Y));
    use ZoomPoint::*;
    let (point, flip) = match control >> 8 & 15 {
        5 => (TopLeft, 0),
        6 => (TopMiddle, 0),
        7 => (TopLeft, H_FLIP),
        9 => (LeftMiddle, 0),
        10 => (Centre, 0),
        11 => (LeftMiddle, H_FLIP),
        13 => (TopLeft, V_FLIP),
        14 => (TopMiddle, V_FLIP),
        15 => (TopLeft, H_FLIP | V_FLIP),
        _ => {
            cpu.write16(entry, (control & !ZOOM_POINT | ZOOM_CENTRE) as u16);
            (Centre, 0)
        }
    };
    if flip != 0 {
        cpu.write16(entry, (control ^ flip) as u16);
    }
    if flip & H_FLIP != 0 {
        (ws, wc) = (ws.wrapping_neg(), wc.wrapping_neg());
    }
    if flip & V_FLIP != 0 {
        (hs, hc) = (hs.wrapping_neg(), hc.wrapping_neg());
    }
    let half = |v: u32| (v as i32 >> 1) as u32;
    let (x0, y0) = match point {
        TopLeft => (x, y),
        TopMiddle => (x.wrapping_sub(half(ws)), y.wrapping_sub(half(wc))),
        LeftMiddle => (x.wrapping_add(half(hc)), y.wrapping_sub(half(hs))),
        Centre => (x.wrapping_sub(half(ws)).wrapping_add(half(hc)), y.wrapping_sub(half(hs)).wrapping_sub(half(wc))),
    };
    let corners = [
        (x0, y0),
        (x0.wrapping_add(ws), y0.wrapping_add(wc)),
        (x0.wrapping_add(ws).wrapping_sub(hc), y0.wrapping_add(wc).wrapping_add(hs)),
        (x0.wrapping_sub(hc), y0.wrapping_add(hs)),
    ];
    let inside = |&(cx, cy): &(u32, u32)| cx.wrapping_add(x_off) <= x_limit && cy.wrapping_add(y_off) <= y_limit;
    if !corners.iter().all(inside) {
        let depth = cpu.read16(entry + ENTRY_DEPTH) as u32;
        let next = cpu.read32(entry + ENTRY_NEXT);
        let table = cpu.read32(w + SLAVE_DEPTHS);
        cpu.write32(table + ((depth >> 8 & 0xff) << 2), next);
        return;
    }
    cpu.write32(w + SLAVE_SLOT, entry + POLYGON_ENTRY);
    let [a, b, c, d] = corners.map(|(cx, cy)| cx & 0xffff_0000 | cy >> 16);
    let first = if cpu.read16(entry + 4) & GOURAUD_SHADED != 0 {
        0
    } else {
        let halves = cpu.read32(w + SLAVE_WIDTH);
        let (hw, hh) = ((halves >> 16) as i16 as i32 as u32, halves as i16 as i32 as u32);
        let size = cpu.read32(w + SLAVE_SIZE);
        let (sw, sh) = (size >> 16, size & 0xffff);
        let on = |p: u32| (p as i16 as i32 as u32).wrapping_add(hh) < sh && sw > ((p >> 16) as i16 as i32 as u32).wrapping_add(hw);
        match [a, b, c, d].iter().position(|&p| on(p)) {
            Some(k) => k,
            None => {
                let away = |v: u32| if v & 0x8000 != 0 { !v } else { v };
                let distance = |p: u32| (away(p >> 16).wrapping_add(away(p))) & 0xffff;
                let (mut best, mut first) = (distance(d), 3);
                for p in [c, b] {
                    if distance(p) <= best {
                        (best, first) = (distance(p), 2);
                    }
                }
                if best >= distance(a) { 0 } else { first }
            }
        }
    };
    let (order, toggle) = match first {
        0 => ([a, b, c, d], 0),
        1 => ([b, a, d, c], H_FLIP),
        2 => ([c, d, a, b], H_FLIP | V_FLIP),
        _ => ([d, c, b, a], V_FLIP),
    };
    if toggle != 0 {
        let now = cpu.read16(entry) as u32;
        cpu.write16(entry, (now ^ toggle) as u16);
    }
    for (k, p) in order.into_iter().enumerate() {
        cpu.write32(entry + CORNERS + 4 * k as u32, p);
    }
}

/// 20: polygon data (the master's slPutPolygon): after the command's first word (the points'
/// count) come the points' address, the polygons' count and address, the attributes'
/// address and the matrix (3 rows of 4 longs). The matrix is kept (SGL's copy in its code); each
/// point, from the last, turned into the eye's frame and put on the screen into the points room
/// (16 bytes a point: the screen place as a pair of words, the eye's x with bit 0 its off-screen
/// flag, the eye's y, its depth no nearer than the near limit), the perspective by the divide
/// unit (half the screen's width over the depth; each point's division started as the one before
/// is finished, so the first pass finishes one past the last point from the room's stale
/// contents, as SGL's pipeline does); an off-screen point, when the first setting's bit 0 asks,
/// drawn in to the ±512 box along its ray. Then each polygon: its normal turned; facing away
/// (the eye's view of it past the second setting) it is dropped unless two-sided, its normal then
/// reversed. Its attribute's kind (the low 7 bits) picks three stages (SGL's table at 0x06037318):
/// the depth (the previous polygon's, the nearest point's, the farthest's or the middle), out
/// past the far limit or nearer than the near; the colour (the texture's address, the attribute's
/// colour, mode and Gouraud table; the corners from the first one on the screen unless Gouraud
/// shaded); the shading. Then the command is linked by its depth and the slot moves on. The place
/// in the commands is left past the matrix (`-> slave`).
const MATRIX_COPY: u32 = 0x0603_6f5c;
const MATRIX_ORDER: [u32; 12] = [0, 4, 8, 16, 20, 24, 32, 36, 40, 12, 28, 44];
const POINT: u32 = 16;
const SOURCE_POINT: u32 = 12;
const SCREEN_LIMIT: i32 = 0x200;
const NORMAL: u32 = 12;
const POLYGON: u32 = 20;
const ATTRIBUTE: u32 = 12;
const DEPTH_CUE: u32 = 0x0603_83cc;
const LIGHT_GOURAUD: u32 = 0xffff_ffe0;
const POLYGON_MATRIX_END: u32 = 64;

/// The middle 32 bits of a sum of three products (MAC's, as SGL takes 16.16 results).
fn mac3(a: [u32; 3], b: [u32; 3]) -> u32 {
    let sum: i64 = (0..3).map(|i| a[i] as i32 as i64 * b[i] as i32 as i64).sum();
    (sum >> 16) as u32
}

/// The high 32 bits of a sum of three products (MACH: the sign and size of a dot product).
fn mac3_high(a: [u32; 3], b: [u32; 3]) -> (u32, u32) {
    let sum: i64 = (0..3).map(|i| a[i] as i32 as i64 * b[i] as i32 as i64).sum();
    ((sum >> 32) as u32, sum as u32)
}

/// The high 32 bits of a product (DMULS's MACH).
fn mul_high(a: u32, b: u32) -> u32 {
    ((a as i32 as i64 * b as i32 as i64) >> 32) as u32
}

/// A screen place outside the ±512 box drawn in along its ray: both halved until inside, then
/// four steps of half the last toward the box's edge, the larger of the two leading. (SGL's loop
/// for x below the box branches back to its bound's negation, so the bound turns between -512
/// and 512 each pass: a negative x is halved twice at a time.)
fn draw_in(a: u32, b: u32) -> (u32, u32) {
    let (mut a, mut b) = (a as i32, b as i32);
    let start = (a, b);
    while a > SCREEN_LIMIT {
        (a, b) = (a >> 1, b >> 1);
    }
    while b > SCREEN_LIMIT {
        (a, b) = (a >> 1, b >> 1);
    }
    let mut bound = SCREEN_LIMIT;
    loop {
        bound = -bound;
        if a >= bound {
            break;
        }
        (a, b) = (a >> 1, b >> 1);
    }
    while b < -SCREEN_LIMIT {
        (a, b) = (a >> 1, b >> 1);
    }
    if (a, b) == start {
        return (a as u32, b as u32);
    }
    let (mut sa, mut sb) = (a, b);
    let b_leads = b.wrapping_abs() >= a.wrapping_abs();
    let ahead = if b_leads { b >= 0 } else { a >= 0 };
    for _ in 0..4 {
        let next = if b_leads { b.wrapping_add(sb) } else { a.wrapping_add(sa) };
        if (ahead && next <= SCREEN_LIMIT) || (!ahead && next >= -SCREEN_LIMIT) {
            a = a.wrapping_add(sa);
            b = b.wrapping_add(sb);
        }
        sa >>= 1;
        sb >>= 1;
    }
    (a as u32, b as u32)
}

/// A point's screen place finished from its eye x and y and the divide unit's quotient: its
/// off-screen flag in the eye x's bit 0 (beyond the screen's size and the margin, either way),
/// stored with the eye y; the place, drawn in when the flag and the first setting's bit 0 say.
fn point_finish(cpu: &mut Cpu, record: u32, x: u32, y: u32, q: u32, size: (u32, u32)) -> u32 {
    let w = cpu.gbr();
    let (width, height) = size;
    let xs = mul_high(x, q);
    let hw = cpu.read16(w + SLAVE_WIDTH) as i16 as i32 as u32;
    cpu.write32(record + 8, y);
    let ys = mul_high(y, q);
    let margin = cpu.read32(w + SLAVE_SETTINGS[2]);
    let x_off = xs.wrapping_add(hw).wrapping_add(margin) >= width;
    let hh = cpu.read16(w + SLAVE_WIDTH + 2) as i16 as i32 as u32;
    let margin = cpu.read32(w + SLAVE_SETTINGS[2]);
    let y_off = ys.wrapping_add(hh).wrapping_add(margin) >= height;
    let flags = (x >> 1 << 1) | x_off as u32 | y_off as u32;
    cpu.write32(record + 4, flags);
    let (xs, ys) = if flags & cpu.read32(w + SLAVE_SETTINGS[0]) & 1 != 0 { draw_in(xs, ys) } else { (xs, ys) };
    (xs << 16) | (ys & 0xffff)
}

pub fn slave_polygons(cpu: &mut Cpu) {
    let w = cpu.gbr();
    let count = cpu.reg(0);
    let at = cpu.reg(4);
    let room = cpu.read32(w + SLAVE_POINTS);
    let end = room + POINT * count;
    // the pipeline's first division: of the depth one past the last point (stale)
    let stale = cpu.read32(end + 12);
    let depth = cpu.read32(w + SLAVE_DEPTH);
    cpu.write32(DVSR, stale);
    let hw = (depth >> 16) as i16 as i32 as u32;
    cpu.write32(DVDNTH, hw);
    let low = depth as i16 as i32 as u32;
    cpu.write32(DVDNTL, low);
    let points = cpu.read32(at);
    cpu.push(&[at + 4]);
    for k in MATRIX_ORDER {
        let v = cpu.read32(at + 16 + k);
        cpu.write32(MATRIX_COPY + k, v);
    }
    if count == 0 {
        // not seen run: SGL's code pops what it never pushed and goes astray
        cpu.leave(&[(4, at + POLYGON_MATRIX_END)]);
        return;
    }
    let row = |cpu: &mut Cpu, r: u32| -> [u32; 4] { std::array::from_fn(|c| cpu.read32(MATRIX_COPY + 16 * r + 4 * c as u32)) };
    cpu.push(&[hw]);
    cpu.write32(w + SLAVE_POINTS, end);
    let level = cpu.read8(w + SLAVE_LEVEL) as u32;
    let near = depth >> (level + 2);
    let size = cpu.read32(w + SLAVE_SIZE);
    cpu.push(&[low]);
    let mut prev = (cpu.read32(end + 4), cpu.read32(end + 8));
    let margin = cpu.read32(w + SLAVE_SETTINGS[2]);
    let screen = ((size >> 16) + 2 * margin, (size & 0xffff) + 2 * margin);
    let mut q = cpu.read32(DVDNTL_AGAIN);
    let mut record = end;
    for i in (0..count).rev() {
        let p: [u32; 3] = std::array::from_fn(|k| cpu.read32(points + SOURCE_POINT * i + 4 * k as u32));
        let [z0, z1, z2, tz] = row(cpu, 2);
        let z = mac3(p, [z0, z1, z2]).wrapping_add(tz);
        cpu.write32(DVDNTH, hw);
        let z = if z as i32 >= near as i32 { z } else { near };
        cpu.write32(DVSR, z);
        record -= POINT;
        cpu.write32(DVDNTL, low);
        cpu.write32(record + 12, z);
        let packed = point_finish(cpu, record + POINT, prev.0, prev.1, q, screen);
        cpu.write32(record + POINT, packed);
        let [x0, x1, x2, tx] = row(cpu, 0);
        let [y0, y1, y2, ty] = row(cpu, 1);
        let x = mac3(p, [x0, x1, x2]).wrapping_add(tx);
        q = cpu.read32(DVDNTL_AGAIN);
        let y = mac3(p, [y0, y1, y2]).wrapping_add(ty);
        prev = (x, y);
    }
    let packed = point_finish(cpu, record, prev.0, prev.1, q, screen);
    cpu.write32(record, packed);
    cpu.pop(3 * 4);
    polygons(cpu, room, at + 4);
}

/// SGL's attribute kinds' colour and shading stages, by the kind's bits 2-5 (its table's second
/// and third columns).
#[derive(Clone, Copy)]
enum Colour {
    /// The attribute's colour, mode and Gouraud table; the corners as below.
    Plain,
    /// The texture's address too.
    Textured,
    /// The corners alone.
    Corners,
    /// Depth cueing (not seen run): the attribute's Gouraud table by the depth (SGL's table at
    /// 0x060383cc), its mode made Gouraud; then the corners.
    DepthCue,
}

#[derive(Clone, Copy)]
enum Shading {
    None,
    /// The attribute's mode, colour and Gouraud table (not seen run).
    Attribute,
    /// The texture's address, then the same (not seen run).
    TexturedAttribute,
    /// Flat light on the attribute's RGB colour through the shade tables (not seen run).
    Flat,
    /// The light's grey Gouraud table for colour modes 1 and 5, the texture's address.
    LightGouraud,
    /// A colour bank's step by the light (not seen run).
    Bank,
}

const STAGES: [(Colour, Shading); 16] = {
    use Colour::*;
    use Shading::*;
    [
        (Plain, None),
        (Textured, None),
        (Corners, Flat),
        (Corners, LightGouraud),
        (DepthCue, Attribute),
        (DepthCue, TexturedAttribute),
        (DepthCue, Flat),
        (DepthCue, LightGouraud),
        (Plain, None),
        (Textured, None),
        (Corners, Bank),
        (Textured, None),
        (DepthCue, Attribute),
        (DepthCue, TexturedAttribute),
        (DepthCue, Bank),
        (DepthCue, TexturedAttribute),
    ]
};

/// The polygons of command 20 (see slave_polygons): their points' records from `points`, the
/// command's words from `place` (the polygons' count and address, the attributes' address).
fn polygons(cpu: &mut Cpu, points: u32, place: u32) {
    let w = cpu.gbr();
    let windows = cpu.read8(w + SLAVE_WINDOW) as i8 as i32 as u32;
    let drew = if windows & 1 != 0 { 32 } else { 16 };
    cpu.write8(w + SLAVE_WINDOW, (windows | drew) as u8);
    let mut slot = cpu.read32(w + SLAVE_SLOT);
    let count = cpu.read32(place);
    let mut polygon = cpu.read32(place + 4);
    let mut attribute = cpu.read32(place + 8);
    let textures = cpu.read32(w + SLAVE_TEXTURES) + 4;
    let depth = cpu.read32(w + SLAVE_DEPTH);
    let level = cpu.read8(w + SLAVE_LEVEL) as u32;
    let near = depth >> 16 >> level;
    cpu.push(&[place + 12]);
    cpu.push(&[textures]);
    cpu.push(&[count]);
    let counter = cpu.stack();
    cpu.push(&[near]);
    // room for the turned normal (not written until it is)
    cpu.sh2.r[15] -= 12;
    let normal_at = cpu.stack();
    let rows: [[u32; 3]; 3] = std::array::from_fn(|r| std::array::from_fn(|c| cpu.read32(MATRIX_COPY + 16 * r as u32 + 4 * c as u32)));
    let mut z = u32::MAX;
    let mut left = count;
    loop {
        let n: [u32; 3] = std::array::from_fn(|k| cpu.read32(polygon + 4 * k as u32));
        let corner = |cpu: &mut Cpu, k: u32| points.wrapping_add(POINT.wrapping_mul(cpu.read16(polygon + NORMAL + 2 * k) as i16 as i32 as u32));
        let p0 = corner(cpu, 0);
        let mut normal = rows.map(|r| mac3(n, r));
        for (k, v) in normal.iter().enumerate() {
            cpu.write32(normal_at + 4 * k as u32, *v);
        }
        let kind_word = cpu.read16(attribute) as i16 as i32 as u32;
        let eye: [u32; 3] = std::array::from_fn(|k| cpu.read32(p0 + 4 + 4 * k as u32));
        let facing = mac3_high(eye, normal).0;
        let control = cpu.read16(attribute + 10);
        cpu.write16(slot, control);
        // the corner stage's Gouraud test reads what is left in r5 when it is not the colour: the
        // stack's place after the normal (r15 + 12, or r15 for a reversed one), unless the depth
        // stage left something else there
        let mut r5 = normal_at + 12;
        let mut drawn = true;
        if facing as i32 >= cpu.read32(w + SLAVE_SETTINGS[1]) as i32 {
            if swap_b(kind_word) & 1 == 0 {
                drawn = false;
            } else {
                normal = normal.map(|v| v.wrapping_neg());
                for (k, v) in normal.iter().enumerate().rev() {
                    cpu.write32(normal_at + 4 * k as u32, *v);
                }
                r5 = normal_at;
            }
        }
        if drawn {
            let corners = [p0, corner(cpu, 1), corner(cpu, 2), corner(cpu, 3)];
            let kind = kind_word & 127;
            let (depth, draw, left_in_r5) = polygon_depth(cpu, kind, corners, z, near, r5);
            z = depth;
            r5 = left_in_r5;
            if draw {
                let (colour, shading) = STAGES[(kind >> 2 & 15) as usize];
                polygon_colour(cpu, colour, slot, attribute, textures, corners, z, r5);
                polygon_shading(cpu, shading, slot, attribute, textures, normal);
                cpu.write16(slot + ENTRY_DEPTH, z as u16);
                let table = cpu.read32(w + SLAVE_DEPTHS);
                let at = table + ((z >> 8 & 0xff) << 2);
                let head = cpu.read32(at);
                cpu.write32(at, slot);
                cpu.write32(slot + ENTRY_NEXT, head);
                slot += POLYGON_ENTRY;
            }
        }
        attribute += ATTRIBUTE;
        polygon += POLYGON;
        left -= 1;
        cpu.write32(counter, left);
        if left == 0 {
            break;
        }
    }
    cpu.pop(24);
    let after = cpu.read32(cpu.stack()) + 48;
    cpu.pop(4);
    cpu.write32(w + SLAVE_SLOT, slot);
    cpu.leave(&[(4, after)]);
}

/// A polygon's depth by its kind's sort (bits 0-1): the previous polygon's (`z`), its nearest
/// point's, its farthest's or the middle of the four; and whether it is drawn: no farther than
/// the far limit past the near (the previous polygon's: no farther than the far limit). The kinds
/// from 64 sort in 16.16, check against the near's half, and drop polygons with all four points
/// off one edge or, unless the first setting's bit 0, a point past ±1024 on the screen. With what
/// the SH-2 code leaves in r5 (the colour stage's Gouraud test may read it): the near limit for
/// the integer sorts, its half in 16.16 for the others, 0x3ff after the ±1024 check, `r5` as it
/// was for the previous polygon's depth.
fn polygon_depth(cpu: &mut Cpu, kind: u32, p: [u32; 4], z: u32, near: u32, r5: u32) -> (u32, bool, u32) {
    let w = cpu.gbr();
    let far = cpu.read16(w + SLAVE_FAR) as i16 as i32 as u32;
    if kind & 3 == 0 {
        return (z, z <= far, r5);
    }
    let wide = kind & 64 != 0;
    let depths: [u32; 4] = if wide {
        p.map(|q| cpu.read32(q + 12))
    } else if kind & 3 == 3 {
        p.map(|q| cpu.read32(q + 12))
    } else {
        p.map(|q| cpu.read16(q + 12) as i16 as i32 as u32)
    };
    let z = match kind & 3 {
        1 => depths.iter().copied().fold(depths[0], |m, d| if m as i32 > d as i32 { d } else { m }),
        2 => depths.iter().copied().fold(depths[0], |m, d| if m as i32 >= d as i32 { m } else { d }),
        _ => {
            let half = |v: u32| (v as i32 >> 1) as u32;
            let mid = half(half(depths[0].wrapping_add(depths[1])).wrapping_add(half(depths[2].wrapping_add(depths[3]))));
            if wide { mid } else { (mid >> 16) as i16 as i32 as u32 }
        }
    };
    if !wide {
        return (z, z.wrapping_sub(near) <= far, near);
    }
    let near = near << 16;
    let half = near >> 1;
    if z > (far << 16).wrapping_add(near) || z as i32 <= half as i32 {
        return (z, false, half);
    }
    let z = z >> 16;
    if p.iter().all(|&q| cpu.read32(q + 4) & 1 != 0) {
        return (z, false, half);
    }
    if cpu.read32(w + SLAVE_SETTINGS[0]) & 1 != 0 {
        return (z, true, half);
    }
    let inside = p.iter().all(|&q| (0..2).all(|k| (-1024..=1023).contains(&(cpu.read16(q + 2 * k) as i16))));
    (z, inside, 0x3ff)
}

/// A polygon's colour stage (see Colour): `r5` what the SH-2 code's corner stage tests for a
/// Gouraud mode when it is entered without the colour.
#[allow(clippy::too_many_arguments)]
fn polygon_colour(cpu: &mut Cpu, colour: Colour, slot: u32, attribute: u32, textures: u32, p: [u32; 4], z: u32, r5: u32) {
    let mut tested = r5;
    match colour {
        Colour::Textured | Colour::Plain => {
            if let Colour::Textured = colour {
                texture_address(cpu, slot, attribute, textures);
            }
            let gouraud = cpu.read16(attribute + 8);
            let mode = cpu.read32(attribute + 4);
            cpu.write16(slot + 28, gouraud);
            cpu.write32(slot + 4, mode);
            tested = mode;
        }
        Colour::Corners => {}
        Colour::DepthCue => {
            let start = cpu.read32(DEPTH_CUE) as i32;
            let range = cpu.read32(DEPTH_CUE + 4) as i32;
            let step = if start >= z as i32 {
                0
            } else if (z as i32).wrapping_sub(start) >= range {
                cpu.read16(DEPTH_CUE + 10) as i16 as i32 as u32
            } else {
                let shift = cpu.read16(DEPTH_CUE + 8) as i16 as i32 as u32;
                (z.wrapping_sub(start as u32)).checked_shr(shift).unwrap_or(0)
            };
            let base = cpu.read16(DEPTH_CUE + 12) as u32;
            cpu.write16(attribute + 8, base.wrapping_add(step) as u16);
            let mode = cpu.read16(attribute + 4);
            cpu.write16(attribute + 4, mode | 4);
            tested = step;
        }
    }
    let screen = p.map(|q| cpu.read32(q));
    let [a, b, c, d] = screen;
    let first = if tested >> 18 & 1 != 0 {
        0
    } else {
        match p.iter().position(|&q| cpu.read32(q + 4) & 1 == 0) {
            Some(k) => k,
            None => {
                let away = |v: u32| if v & 0x8000 != 0 { !v } else { v };
                let distance = |v: u32| (away(v >> 16).wrapping_add(away(v))) & 0xffff;
                let (mut best, mut first) = (distance(d), 3);
                for (k, v) in [(2, c), (1, b)] {
                    if distance(v) <= best {
                        (best, first) = (distance(v), k);
                    }
                }
                if best >= distance(a) { 0 } else { first }
            }
        }
    };
    let (order, toggle) = match first {
        0 => ([a, b, c, d], 0),
        1 => ([b, a, d, c], 16),
        2 => ([c, d, a, b], 48),
        _ => ([d, c, b, a], 32),
    };
    if toggle != 0 {
        let now = cpu.read16(slot) as u32;
        cpu.write16(slot, (now ^ toggle) as u16);
    }
    for (k, v) in order.into_iter().enumerate() {
        cpu.write32(slot + 12 + 4 * k as u32, v);
    }
}

/// The texture's address (the attribute's texture number into SGL's texture table) for the command.
fn texture_address(cpu: &mut Cpu, slot: u32, attribute: u32, textures: u32) {
    let number = cpu.read16(attribute + 2) as i16 as i32 as u32;
    let address = cpu.read32(textures.wrapping_add(number.wrapping_mul(8)));
    cpu.write32(slot + 8, address);
}

/// A polygon's shading stage (see Shading), with its normal (turned, reversed for a back face).
fn polygon_shading(cpu: &mut Cpu, shading: Shading, slot: u32, attribute: u32, textures: u32, normal: [u32; 3]) {
    let w = cpu.gbr();
    let light = |cpu: &mut Cpu| -> (u32, u32) {
        let l: [u32; 3] = std::array::from_fn(|k| cpu.read32(w + SLAVE_LIGHT + 4 * k as u32));
        mac3_high(normal, l)
    };
    match shading {
        Shading::None => {}
        Shading::TexturedAttribute | Shading::Attribute => {
            if let Shading::TexturedAttribute = shading {
                texture_address(cpu, slot, attribute, textures);
            }
            let gouraud = cpu.read16(attribute + 8);
            let mode = cpu.read32(attribute + 4);
            cpu.write16(slot + 28, gouraud);
            cpu.write32(slot + 4, mode);
        }
        Shading::Flat => {
            let (high, low) = light(cpu);
            let colour = cpu.read16(attribute + 6) as i16 as i32 as u32;
            let (red, green, blue) = (colour & 31, colour >> 5 & 31, colour >> 10 & 31);
            let mut row = cpu.read32(w + SLAVE_SHADES);
            let step = match high {
                0 => Some((low >> 25) | 0x80),
                u32::MAX => Some(low >> 25),
                h if h as i32 > 0 => Some(0xff),
                _ => None,
            };
            if let Some(step) = step {
                row = row.wrapping_add((step & !7) << 2);
            }
            let shade = |cpu: &mut Cpu, plane: u32, v: u32| cpu.read8(row + plane * SHADE_PLANE + v) as i8 as i32 as u32;
            let (r, g, b) = (shade(cpu, 0, red), shade(cpu, 1, green), shade(cpu, 2, blue));
            let lit = (swap_b(b | 32).wrapping_add(g << 3) << 2) | r;
            let mode = cpu.read16(attribute + 4) as u32;
            cpu.write32(slot + 4, mode << 16 | (lit & 0xffff));
            let gouraud = cpu.read16(attribute + 8);
            cpu.write16(slot + 28, gouraud);
        }
        Shading::Bank => {
            let mode = cpu.read32(attribute + 4);
            let (high, low) = light(cpu);
            let step = match high {
                0 => Some(low >> 29 | 8),
                u32::MAX => Some(low >> 29),
                h if h as i32 > 0 => Some(15),
                _ => None,
            };
            cpu.write32(slot + 4, mode.wrapping_add(step.unwrap_or(0)));
        }
        Shading::LightGouraud => {
            texture_address(cpu, slot, attribute, textures);
            let (high, low) = light(cpu);
            let mode = cpu.read32(attribute + 4);
            let colour_mode = mode >> 16 & 56;
            if colour_mode == 8 || colour_mode == 40 {
                let step = match high {
                    0 => Some(low >> 28 | 16),
                    u32::MAX => Some(low >> 28),
                    h if h as i32 > 0 => Some(31),
                    _ => None,
                };
                cpu.write16(slot + 28, LIGHT_GOURAUD.wrapping_add(step.unwrap_or(0)) as u16);
                let gouraud_mode = (mode.rotate_left(16) & !7 | 4).rotate_left(16);
                cpu.write32(slot + 4, gouraud_mode);
            } else {
                cpu.write32(slot + 4, mode);
                let gouraud = cpu.read16(attribute + 8);
                cpu.write16(slot + 28, gouraud);
            }
        }
    }
}


/// 4: the frame's end on the slave: SGL's draw list (the SCU's indirect DMA table at
/// 0x060089bc's address: 12-byte entries, each copying a command to VDP1, its source at +8) made
/// from the depth lists (at 0x060089c0's address: window 0's 128 from the farthest at its end
/// down, then, when window 1 drew, its clipping commands and window 1's 128 below): each list's
/// commands sorted into 256 buckets after the lists by their fine depth (the byte at +31), the
/// deepest first, and the list and its buckets emptied. Then the list's end (SGL's end command,
/// its source's bit 31 the table's last), the frame drawn, the commands counted (from the end's
/// place in VDP1's table, less the three system commands), the window's clip and local
/// coordinates into VDP1's system commands, the next slot and the cache purged. With SGL's
/// dynamic frame (the screen mode's bit 7, set in play) the list starts with the command at
/// 0x060364cc (a scaled sprite over the whole screen: the frame's erase, as far as can be told).
/// The slave's registers left as the SH-2 code leaves them (r8-r12 its walk's, r4 back on the
/// word after the command).
const DEPTH_HALF: u32 = 0x400;
const LISTS_A_WINDOW: u32 = 128;
const DMA_ENTRY: u32 = 12;
const DMA_SOURCE: u32 = 8;
const ERASE_COMMAND: u32 = 0x0603_64cc;
const WINDOW_1_CLIPS: [u32; 2] = [0x060f_ff00, 0x060f_ff2c];
const END_COMMAND: u32 = 0x8603_6bbe;
const SCREEN_MODE_CT: u32 = 0x260f_fcb0;
const FINE_DEPTH: u32 = 30;
const NEXT_COMMAND: u32 = 32;
const WINDOW_1_DREW: u8 = 32;
const SLAVE_COMMAND_COUNT: u32 = 0xa4;
const SYSTEM_COMMANDS: u32 = 3;
const VDP1_SYSTEM_CLIP: u32 = 0x25c0_0020;
const CLIP_COPIES: [(u32, u32); 5] = [(0, 0), (12, 12), (20, 20), (44, 32), (56, 44)];
const CACHE_PURGE_ON: u8 = 0x11;

pub fn slave_frame_draw(cpu: &mut Cpu) {
    let w = cpu.gbr();
    let mut depth = cpu.read32(SLAVE_BUFFER_AREA) + DEPTH_HALF;
    let mut list = cpu.read32(DRAW_LIST);
    let buckets = depth;
    let dynamic = cpu.read8(SCREEN_MODE_CT) & 0x80 != 0;
    depth -= 4;
    if dynamic {
        cpu.write32(list + DMA_SOURCE, ERASE_COMMAND);
        list += DMA_ENTRY;
    }
    let mut r8;
    loop {
        let mut left = LISTS_A_WINDOW;
        r8 = cpu.read32(depth);
        loop {
            depth -= 4;
            left -= 1;
            if r8 != 0 {
                // the list sorted into the buckets by fine depth, its deepest bucket kept
                let (mut deepest, mut count, mut command) = (0, 0u32, r8);
                loop {
                    let bucket = 4 * (cpu.read16(command + FINE_DEPTH) as u8 as u32);
                    let next = cpu.read32(command + NEXT_COMMAND);
                    let head = cpu.read32(buckets + bucket);
                    deepest = deepest.max(bucket);
                    cpu.write32(command + NEXT_COMMAND, head);
                    count += 1;
                    cpu.write32(buckets + bucket, command);
                    command = next;
                    if next == 0 {
                        break;
                    }
                }
                cpu.write32(depth + 4, 0);
                // the buckets' commands into the draw list, the deepest first
                let mut bucket = buckets + deepest;
                loop {
                    let mut c = cpu.read32(bucket);
                    bucket -= 4;
                    if c == 0 {
                        continue;
                    }
                    while c != 0 {
                        let next = cpu.read32(c + NEXT_COMMAND);
                        cpu.write32(list + DMA_SOURCE, c);
                        list += DMA_ENTRY;
                        c = next;
                        count -= 1;
                    }
                    cpu.write32(bucket + 4, 0);
                    if count == 0 {
                        break;
                    }
                }
            }
            r8 = cpu.read32(depth);
            if left == 0 {
                break;
            }
        }
        if cpu.read8(w + SLAVE_WINDOW) & WINDOW_1_DREW == 0 {
            break;
        }
        cpu.write8(w + SLAVE_WINDOW, 0);
        cpu.write32(w + SLAVE_DEPTHS, depth + 4);
        cpu.write32(list + DMA_SOURCE, WINDOW_1_CLIPS[0]);
        cpu.write32(list + DMA_ENTRY + DMA_SOURCE, WINDOW_1_CLIPS[1]);
        list += 2 * DMA_ENTRY;
    }
    cpu.write32(list + DMA_SOURCE, END_COMMAND);
    let end = cpu.read32(list + 4);
    cpu.write8(w + SYNCH_A, 1);
    cpu.write16(w + SLAVE_COMMAND_COUNT, ((end & 0xffff) >> 5).wrapping_sub(SYSTEM_COMMANDS) as u16);
    cpu.write8(w + SYNCH_B, 0xff);
    for (from, to) in CLIP_COPIES {
        let v = cpu.read32(SLAVE_WINDOWS + from);
        cpu.write32(VDP1_SYSTEM_CLIP + to, v);
    }
    let base = cpu.read32(w + BUFFER_SIDE);
    let area = cpu.read32(POLYGON_AREA);
    cpu.write32(w + SLAVE_SLOT, base.wrapping_add(area));
    cpu.write8(CCR, CACHE_PURGE_ON);
    let r4 = cpu.reg(4) - 4;
    cpu.leave(&[(0, CACHE_PURGE_ON as u32), (1, CCR - 2), (2, area), (4, r4), (6, SLAVE_WINDOWS), (7, VDP1_SYSTEM_CLIP)]);
    cpu.leave(&[(8, r8), (9, 0), (10, depth), (11, list), (12, buckets)]);
    cpu.leave_t(true);
}


/// 8: the slave's screen set up for TV mode `mode` (the word after the command, by its bits
/// 2-6): half the screen's width and height from SGL's table of them, the screen's halves and
/// size, the local coordinates command's centre (its head after: command 10), the erase sprite's
/// corner and size, both windows' clip size; the user clip command's head (command 8) and corner
/// reset; both windows at level 1, far limit 0x3fff and depth half the width (the default
/// perspective); the window flags cleared, the three settings 1, 0, 0; the slot and the points
/// room at SGL's areas, the light straight on (0, 0, -1), the buffer side cleared; the depth lists
/// and their buckets cleared (window 0's lists the upper half of the first 0x400 bytes), the shade
/// tables made; the draw list's DMA entries laid out (30 bytes each, to VDP1's commands from the
/// fourth, as far as the list goes), the cache purged. The slave's registers left as the SH-2 code
/// leaves them (r8-r14 untouched).
const LOCAL_COMMAND: u32 = 0x060f_ff28;
const LOCAL_HEAD: u32 = 0x100a_000c;
const LOCAL_MODE: u16 = 10;
const CLIP_HEAD: u32 = 0x1008_0008;
const CLIP_MODE: u16 = 8;
const COMMAND_A: u32 = 12;
const COMMAND_B: u32 = 16;
const SCREEN_ORIGIN: (u32, u32) = (0x140, 0x200);
const W_SIZE: u32 = 20;
const W_FAR: u32 = 28;
const FAR_DEFAULT: u16 = 0x3fff;
const LIGHT_AHEAD: u32 = 0xffff_0000;
const SLAVE_SHADES: u32 = 0x64;
const SHADE_AREA: u32 = 0x0600_89cc;
const DRAW_LIST_SIZE: u32 = 0x0600_4064;
const VDP1_FOURTH_COMMAND: u32 = 0x25c0_0060;
const VDP1_COMMAND: u32 = 32;
const COMMAND_BYTES: u32 = 30;
const CLEAR_PAIRS: u32 = 0x0603_61ec;
const SHADE_TABLES: u32 = 0x0603_61fe;

pub fn slave_screen(cpu: &mut Cpu) {
    let w = cpu.gbr();
    let at = SCREEN_HALVES + (cpu.reg(0) & 124);
    let half_w = cpu.read16(at) as i16 as i32 as u32;
    let half_h = cpu.read16(at + 2) as i16 as i32 as u32;
    let pair = |x: u32, y: u32| x << 16 | y & 0xffff;
    let halves = pair(half_w, half_h);
    cpu.write32(LOCAL_COMMAND + COMMAND_A, halves);
    cpu.write32(LOCAL_COMMAND + COMMAND_B, halves);
    cpu.write32(w + SLAVE_WIDTH, halves);
    cpu.write32(w + SLAVE_SIZE, halves.wrapping_add(halves));
    cpu.write32(ERASE_COMMAND + COMMAND_A, pair(half_w.wrapping_neg(), half_h.wrapping_neg()));
    cpu.write32(w + SLAVE_DEPTH, half_w.rotate_left(16));
    cpu.write16(LOCAL_COMMAND + 4, LOCAL_MODE);
    cpu.write32(LOCAL_COMMAND, LOCAL_HEAD);
    cpu.write32(w + SLAVE_SCREEN, pair(SCREEN_ORIGIN.0 + half_w, SCREEN_ORIGIN.1 + half_h));
    let size = pair(2 * half_w - 1, 2 * half_h - 1);
    for at in [SLAVE_WINDOWS + W_SIZE, SLAVE_WINDOWS + W_SIZE + 4, ERASE_COMMAND + COMMAND_B] {
        cpu.write32(at, size);
    }
    cpu.write8(w + SLAVE_LEVEL, 1);
    for k in [0, 4] {
        cpu.write8(SLAVE_WINDOWS + W_LEVEL_BYTE + k, 1);
    }
    cpu.write16(SLAVE_WINDOWS + 4, CLIP_MODE);
    cpu.write32(SLAVE_WINDOWS, CLIP_HEAD);
    cpu.write16(w + SLAVE_FAR, FAR_DEFAULT);
    for k in [0, 4] {
        cpu.write16(SLAVE_WINDOWS + W_FAR + k, FAR_DEFAULT);
    }
    let depth = cpu.read32(w + SLAVE_DEPTH);
    for k in [0, 4] {
        cpu.write32(SLAVE_WINDOWS + W_DEPTH + k, depth);
    }
    for field in [COMMAND_A, COMMAND_B] {
        cpu.write32(SLAVE_WINDOWS + field, 0);
    }
    cpu.write8(w + SLAVE_WINDOW, 0);
    cpu.write32(w + SLAVE_SETTINGS[1], 0);
    cpu.write32(w + SLAVE_SETTINGS[2], 0);
    cpu.write32(w + SLAVE_SETTINGS[0], 1);
    let polygons = cpu.read32(POLYGON_AREA);
    cpu.write32(w + SLAVE_SLOT, polygons);
    let points = cpu.read32(POINT_AREA);
    cpu.write32(w + SLAVE_POINTS, points);
    for field in [SLAVE_LIGHT, SLAVE_LIGHT + 4, BUFFER_SIDE] {
        cpu.write32(w + field, 0);
    }
    cpu.write32(w + SLAVE_LIGHT + 8, LIGHT_AHEAD);
    let lists = cpu.read32(SLAVE_BUFFER_AREA);
    let pr = cpu.pr();
    cpu.push(&[pr]);
    for i in 0..4 {
        cpu.leave(&[(2, lists + 0x200 * i)]);
        if i == 1 {
            cpu.write32(w + SLAVE_DEPTHS, lists + 0x200);
        }
        cpu.call(CLEAR_PAIRS, &[]);
    }
    let shades = cpu.read32(SHADE_AREA);
    cpu.write32(w + SLAVE_SHADES, shades);
    cpu.leave(&[(0, shades)]);
    cpu.call(SHADE_TABLES, &[]);
    let mut entry = cpu.read32(DRAW_LIST);
    let end = cpu.read32(DRAW_LIST_SIZE).wrapping_add(entry);
    let mut to = VDP1_FOURTH_COMMAND;
    loop {
        cpu.write32(entry + 4, to);
        cpu.write32(entry, COMMAND_BYTES);
        entry += DMA_ENTRY;
        to += VDP1_COMMAND;
        if entry as i32 >= end as i32 {
            break;
        }
    }
    cpu.pop(4);
    cpu.write8(CCR, CACHE_PURGE_ON);
    cpu.leave(&[(0, CACHE_PURGE_ON as u32), (1, CCR - 2), (2, COMMAND_BYTES), (3, entry)]);
    cpu.leave_t(true);
}


/// 0x060361ec (slave_screen's): 64 pairs of longs from r2 on cleared, r2 left past them.
pub fn slave_clear_pairs(cpu: &mut Cpu) {
    let at = cpu.reg(2);
    for i in 0..64 {
        cpu.write32(at + 8 * i, 0);
        cpu.write32(at + 8 * i + 4, 0);
    }
    cpu.leave(&[(0, 0), (1, 0), (2, at + 64 * 8)]);
    cpu.leave_t(true);
}


/// 0x060361fe (slave_screen's): SGL's shade tables at r0: for each of 32 light levels k, each of
/// 32 shades j as j + k - 16 (0-31), in three planes 0x400 apart; then the grey Gouraud tables at
/// the end of VDP1's VRAM (32 of four words, the i-th grey i * 0x421).
const SHADE_PLANE: u32 = 0x400;
const GREY_GOURAUD: u32 = 0x25c7_ff00;

pub fn slave_shade_tables(cpu: &mut Cpu) {
    let mut at = cpu.reg(0);
    for level in 0..32i32 {
        for shade in 0..32i32 {
            let v = (shade + level - 16).clamp(0, 31) as u8;
            for plane in 0..3 {
                cpu.write8(at + plane * SHADE_PLANE, v);
            }
            at += 1;
        }
    }
    for i in 0..32 {
        for k in 0..4 {
            cpu.write16(GREY_GOURAUD + 8 * i + 2 * k, (i * GREY_STEP) as u16);
        }
    }
    cpu.leave(&[(0, 32 * GREY_STEP), (1, 0), (2, GREY_STEP), (3, GREY_GOURAUD + 32 * 8), (5, at), (6, 31), (7, 31)]);
    cpu.leave_t(true);
}


/// SGL's drawing set-up (slInitSystem's, with the TV mode `mode` in r4 and the texture table in
/// r5): the command area begun at its start (the slave's place and the master's), VDP1's first
/// command an end; the screen mode's low bits the mode's (bit 7 kept unless the mode's bit 4);
/// VDP1's TV mode (8-bit pixels with the mode's bit 3); the slave's screen set-up queued
/// (command 8 with the mode's index) and the slave signalled; the texture table; the master's
/// window from the mode's half width and height (depth, corners, centre, size: VDP1's system
/// clip the size too); the three slave settings' copies 1, 0, 0; VDP1's second command an end;
/// the polygon buffer at the buffer's flip side, the points room at SGL's area; VDP1's first
/// command a system clip; VDP1's frame change, plot trigger (at the frame change) and erase
/// colour and area; the window's limits from the game's (0x06004058, 0x0600405a), far limit
/// 0x3fff; the counts and drawn flag cleared, the light straight on (0, 0, -1).
const VDP1_REGISTERS: u32 = 0x25d0_0000;
const VDP1_END: u16 = 0x8000;
const MASTER_SETTINGS: [u32; 3] = [0x378, 0x380, 0x388];
const SCREEN_COMMAND: u32 = 8;
const SCREEN_PLACE: u32 = 0x0140_0200;
const SCREEN_PLACE_SWAPPED: u32 = 0x0200_0140;
const SYSTEM_CLIP: u32 = 0x1009_0004;
const VDP1_PLOT_AT_CHANGE: u16 = 2;
const ERASE_AREA: u16 = 0x58ff;
const GAME_LIMITS: [u32; 2] = [0x0600_4058, 0x0600_405a];
const ONE_ONE: u32 = 0x1_0001;

pub fn draw_init(cpu: &mut Cpu) {
    let w = cpu.gbr();
    let (mode, textures) = (cpu.reg(4) & 31, cpu.reg(5));
    let area = cpu.read32(COMMAND_AREA) | CACHE_THROUGH;
    cpu.write32(w + SLAVE_PLACE, area);
    cpu.write32(w + COMMANDS, area);
    cpu.write16(VDP1_VRAM, VDP1_END);
    let mut screen = cpu.read8(w + SCREEN_MODE) as i8 as i32 as u32 & 0x80 | mode;
    let t = screen & 16 == 0;
    if !t {
        screen &= 0x7f;
    }
    cpu.write8(w + SCREEN_MODE, screen as u8);
    cpu.write16(VDP1_REGISTERS, (screen >> 3 & 1) as u16);
    let index = mode << 2;
    cpu.write32(area + 4, index);
    cpu.write32(area, SCREEN_COMMAND);
    cpu.write32(w + COMMANDS, area + 8);
    cpu.write16(SLAVE_SIGNAL, (area + 8) as u16);
    cpu.write32(w + SLAVE_TEXTURES, textures);
    let halves = cpu.read32(SCREEN_HALVES + index);
    cpu.write32(w + WIN_DEPTH, halves.rotate_left(16) << 16);
    cpu.write32(w + WIN_LEFT_TOP, halves.wrapping_neg().wrapping_add(1 << 16));
    cpu.write32(w + WIN_SCREEN_X, SCREEN_PLACE.wrapping_add(halves));
    cpu.write32(w + WIN_WIDTH, halves);
    cpu.write32(w + WIN_RIGHT_TOP, halves.wrapping_sub(ONE_ONE));
    let size = halves.wrapping_add(halves);
    cpu.write32(w + SCREEN_WIDTH, size);
    cpu.write32(w + WIN_SIZE, size);
    cpu.write32(VDP1_VRAM + 20, size.wrapping_sub(ONE_ONE));
    cpu.write32(w + MASTER_SETTINGS[1], 0);
    cpu.write32(w + MASTER_SETTINGS[2], 0);
    cpu.write32(w + MASTER_SETTINGS[0], 1);
    cpu.write32(w + WIN_SCREEN, SCREEN_PLACE_SWAPPED);
    // the area's address in it too, as the SH-2 code has it
    let mixed = (area << 16 | SCREEN_PLACE_SWAPPED).wrapping_mul(2);
    cpu.write32(w + WIN_SIZE_SCREEN, size.wrapping_add(mixed));
    cpu.write16(VDP1_VRAM + VDP1_COMMAND, VDP1_END);
    let polygons = cpu.read32(POLYGON_AREA);
    let flip = cpu.read32(BUFFER_FLIP) >> 1;
    cpu.write32(w + POLYGON_BUFFER, polygons.wrapping_sub(POLYGON_ENTRY).wrapping_add(flip));
    cpu.write32(w + BUFFER_SIDE, 0);
    cpu.write32(VDP1_VRAM, SYSTEM_CLIP);
    cpu.write16(VDP1_REGISTERS + 2, 0);
    cpu.write16(VDP1_REGISTERS + 4, VDP1_PLOT_AT_CHANGE);
    cpu.write16(w + CUR_WINDOW, 0);
    cpu.write16(VDP1_REGISTERS + 6, 0);
    cpu.write16(VDP1_REGISTERS + 8, 0);
    cpu.write16(VDP1_REGISTERS + 10, ERASE_AREA);
    let points = cpu.read32(POINT_AREA);
    let x = cpu.read16(GAME_LIMITS[0]) as i16 as i32 as u32;
    let y = cpu.read16(GAME_LIMITS[1]) as i16 as i32 as u32;
    cpu.write32(w + POINTS_ROOM, points);
    cpu.write32(w + LIMITS, y << 16 | x & 0xffff);
    cpu.write16(w + FAR, 0x3fff);
    cpu.write8(w + SYNCH_A, 0);
    for field in [COUNTS, LIGHT, LIGHT + 4] {
        cpu.write32(w + field, 0);
    }
    cpu.write8(w + SYNCH_B, 0xff);
    cpu.write32(w + LIGHT + 8, 0xffff_0000);
    cpu.write8(w + SYNCH_B, 1);
    cpu.write8(w + WIN_FLAG, 1);
    cpu.leave(&[(0, 1), (1, POINT_AREA), (2, x), (3, y), (4, index), (6, VDP1_REGISTERS), (7, VDP1_VRAM)]);
    cpu.leave_t(t);
}

/// SGL's PCM streams, each VBlank (8 slots of 20 bytes, by each one's state: its high bits pick
/// SGL's step for it from a table): a playing stream's next stretch copied into sound RAM (by
/// the SCU's level-2 DMA or the CPU's channel 1, 0x06033884 and 0x060338da), its play and stop
/// sent; an idle slot passed by. Not seen run past the idle step: the game has no PCM streams.
/// SGL's code block by block (asm2sm.py).
pub fn pcm_streams(cpu: &mut Cpu) {
    pcm_streams_blocks(cpu);
}

#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
fn pcm_streams_blocks(cpu: &mut Cpu) {
    let w = cpu.gbr();
    let [mut r0, mut r1, mut r2, mut r3, mut r4, mut r5, mut r6, mut r7, mut r8, mut r9, mut r10, mut r11, mut r12, mut r13, _, mut r15] = cpu.sh2.r;
    let mut r14 = cpu.caller_r14;
    let mut mac = ((cpu.sh2.mach as u64) << 32 | cpu.sh2.macl as u64) as i64;
    let mut t = cpu.sh2.sr & 1 != 0;
    let entry_pr = cpu.sh2.pr;
    let mut pr = entry_pr;
    macro_rules! hand_over { () => { cpu.sh2.r = [r0, r1, r2, r3, r4, r5, r6, r7, r8, r9, r10, r11, r12, r13, r14, r15]; cpu.leave_mac(mac); cpu.leave_t(t); } }
    macro_rules! take_back { () => { [r0, r1, r2, r3, r4, r5, r6, r7, r8, r9, r10, r11, r12, r13, r14, r15] = cpu.sh2.r; mac = ((cpu.sh2.mach as u64) << 32 | cpu.sh2.macl as u64) as i64; t = cpu.sh2.sr & 1 != 0; } }
    let mut pc: u32 = 0x06033658;
    loop {
        match pc {
            0x06033658 => {
                r15 = r15.wrapping_sub(4); cpu.write32(r15, r8); // 06033658 mov.l r8,@-r15
                r15 = r15.wrapping_sub(4); cpu.write32(r15, pr); // 0603365a sts.l pr,@-r15
                r15 = r15.wrapping_sub(4); cpu.write32(r15, r9); // 0603365c mov.l r9,@-r15
                r8 = 0x06034f48; // 0603365e mov.l 0x6033938,r8
                r9 = 8i32 as u32; // 06033660 mov #8,r9
                pc = 0x06033662;
            }
            0x06033662 => {
                r0 = cpu.read8(r8.wrapping_add(0)) as i8 as i32 as u32; // 06033662 mov.b @(0,r8),r0
                r1 = 124i32 as u32; // 06033664 mov #124,r1
                r1 &= r0; // 06033666 and r0,r1
                r0 = 0x0603393c; // 06033668 mova 0x603393c,r0
                r2 = cpu.read32(r0.wrapping_add(r1)); // 0603366a mov.l @(r0,r1),r2
                r0 = 60i32 as u32; // 0603366c mov #60,r0
                let to = r2; // 0603366e jmp @r2
                r1 &= r0; // 06033670 and r0,r1
                pc = to;
            }
            0x06033672 => {
                r0 = cpu.read8(r8.wrapping_add(1)) as i8 as i32 as u32; // 06033672 mov.b @(1,r8),r0
                r2 = 0x25a007a0; // 06033674 mov.l 0x60339ec,r2
                r3 = r0; // 06033676 mov r0,r3
                r0 >>= 2; // 06033678 shlr2 r0
                t = r0 & 1 != 0; r0 >>= 1; // 0603367a shlr r0
                r0 &= 14; // 0603367c and #14,r0
                r2 = cpu.read8(r0.wrapping_add(r2)) as i8 as i32 as u32; // 0603367e mov.b @(r0,r2),r2
                r0 = 0x060339bc; // 06033680 mova 0x60339bc,r0
                t = r2 & r2 == 0; // 06033682 tst r2,r2
                let take = !t; // 06033684 bf/s 0x6033732
                r7 = 1i32 as u32; // 06033686 mov #1,r7
                pc = if take { 0x06033732 } else { 0x06033688 };
            }
            0x06033688 => {
                r1 = cpu.read16(r0.wrapping_add(r1)) as i16 as i32 as u32; // 06033688 mov.w @(r0,r1),r1
                r5 = cpu.read32(r8.wrapping_add(8)); // 0603368a mov.l @(8,r8),r5
                r4 = cpu.read32(r8.wrapping_add(16)); // 0603368c mov.l @(16,r8),r4
                r6 = cpu.read32(r8.wrapping_add(12)); // 0603368e mov.l @(12,r8),r6
                r0 = r3; // 06033690 mov r3,r0
                r3 = r6; // 06033692 mov r6,r3
                t = r0 & 128 == 0; // 06033694 tst #128,r0
                let take = t; // 06033696 bt/s 0x60336a0
                r2 = 32i32 as u32; // 06033698 mov #32,r2
                pc = if take { 0x060336a0 } else { 0x0603369a };
            }
            0x0603369a => {
                t = r7 >> 31 != 0; r7 <<= 1; // 0603369a shll r7
                t = r2 >> 31 != 0; r2 <<= 1; // 0603369c shll r2
                t = r1 >> 31 != 0; r1 <<= 1; // 0603369e shll r1
                pc = 0x060336a0;
            }
            0x060336a0 => {
                r7 <<= 8; // 060336a0 shll8 r7
                r6 = r6.wrapping_sub(r4); // 060336a2 sub r4,r6
                t = r6 >= r7; // 060336a4 cmp/hs r7,r6
                let take = t; // 060336a6 bt/s 0x60336d0
                r4 = cpu.read32(r8.wrapping_add(4)); // 060336a8 mov.l @(4,r8),r4
                pc = if take { 0x060336d0 } else { 0x060336aa };
            }
            0x060336aa => {
                t = r0 & 2 == 0; // 060336aa tst #2,r0
                let take = t; // 060336ac bt/s 0x60336c4
                r5 = r5.wrapping_add(r1); // 060336ae add r1,r5
                pc = if take { 0x060336c4 } else { 0x060336b0 };
            }
            0x060336b0 => {
                r15 = r15.wrapping_sub(4); cpu.write32(r15, r7); // 060336b0 mov.l r7,@-r15
                r15 = r15.wrapping_sub(4); cpu.write32(r15, r5); // 060336b2 mov.l r5,@-r15
                r15 = r15.wrapping_sub(4); cpu.write32(r15, r4); // 060336b4 mov.l r4,@-r15
                r2 <<= 8; // 060336b6 shll8 r2
                r4 = r4.wrapping_add(r3); // 060336b8 add r3,r4
                let to = 0x06033884; // 060336ba bsr 0x6033884
                r5 = r5.wrapping_add(r2); // 060336bc add r2,r5
                hand_over!(); cpu.call(to, &[]); take_back!();
                pr = 0x060336be;
                pc = 0x060336be;
            }
            0x060336be => {
                r4 = cpu.read32(r15); r15 = r15.wrapping_add(4); // 060336be mov.l @r15+,r4
                r5 = cpu.read32(r15); r15 = r15.wrapping_add(4); // 060336c0 mov.l @r15+,r5
                r7 = cpu.read32(r15); r15 = r15.wrapping_add(4); // 060336c2 mov.l @r15+,r7
                pc = 0x060336c4;
            }
            0x060336c4 => {
                let to = 0x06033884; // 060336c4 bsr 0x6033884
                // nop // 060336c6 nop 
                hand_over!(); cpu.call(to, &[]); take_back!();
                pr = 0x060336c8;
                pc = 0x060336c8;
            }
            0x060336c8 => {
                r2 = cpu.read32(r8.wrapping_add(16)); // 060336c8 mov.l @(16,r8),r2
                r0 = 92i32 as u32; // 060336ca mov #92,r0
                // 060336cc bra 0x603372e
                r2 = r2.wrapping_add(r7); // 060336ce add r7,r2
                pc = 0x0603372e;
            }
            0x060336d0 => {
                r15 = r15.wrapping_sub(4); cpu.write32(r15, r1); // 060336d0 mov.l r1,@-r15
                r1 = r7; // 060336d2 mov r7,r1
                r7 = r7.wrapping_add(r1); // 060336d4 add r1,r7
                r7 = r7.wrapping_add(r1); // 060336d6 add r1,r7
                // 060336d8 bra 0x60336fe
                r1 = cpu.read32(r15); r15 = r15.wrapping_add(4); // 060336da mov.l @r15+,r1
                pc = 0x060336fe;
            }
            0x060336dc => {
                r0 = 0x060339bc; // 060336dc mova 0x60339bc,r0
                r1 = cpu.read16(r0.wrapping_add(r1)) as i16 as i32 as u32; // 060336de mov.w @(r0,r1),r1
                r0 = cpu.read8(r8.wrapping_add(1)) as i8 as i32 as u32; // 060336e0 mov.b @(1,r8),r0
                r5 = cpu.read32(r8.wrapping_add(8)); // 060336e2 mov.l @(8,r8),r5
                r4 = cpu.read32(r8.wrapping_add(16)); // 060336e4 mov.l @(16,r8),r4
                r6 = cpu.read32(r8.wrapping_add(12)); // 060336e6 mov.l @(12,r8),r6
                r7 = 3i32 as u32; // 060336e8 mov #3,r7
                r3 = r6; // 060336ea mov r6,r3
                r6 = r6.wrapping_sub(r4); // 060336ec sub r4,r6
                r4 = cpu.read32(r8.wrapping_add(4)); // 060336ee mov.l @(4,r8),r4
                r7 <<= 8; // 060336f0 shll8 r7
                t = r0 & 128 == 0; // 060336f2 tst #128,r0
                let take = t; // 060336f4 bt/s 0x60336fe
                r2 = 32i32 as u32; // 060336f6 mov #32,r2
                pc = if take { 0x060336fe } else { 0x060336f8 };
            }
            0x060336f8 => {
                t = r7 >> 31 != 0; r7 <<= 1; // 060336f8 shll r7
                t = r2 >> 31 != 0; r2 <<= 1; // 060336fa shll r2
                t = r1 >> 31 != 0; r1 <<= 1; // 060336fc shll r1
                pc = 0x060336fe;
            }
            0x060336fe => {
                t = r0 & 2 == 0; // 060336fe tst #2,r0
                let take = t; // 06033700 bt/s 0x6033718
                r5 = r5.wrapping_add(r1); // 06033702 add r1,r5
                pc = if take { 0x06033718 } else { 0x06033704 };
            }
            0x06033704 => {
                r15 = r15.wrapping_sub(4); cpu.write32(r15, r7); // 06033704 mov.l r7,@-r15
                r15 = r15.wrapping_sub(4); cpu.write32(r15, r5); // 06033706 mov.l r5,@-r15
                r15 = r15.wrapping_sub(4); cpu.write32(r15, r4); // 06033708 mov.l r4,@-r15
                r2 <<= 8; // 0603370a shll8 r2
                r4 = r4.wrapping_add(r3); // 0603370c add r3,r4
                let to = 0x06033884; // 0603370e bsr 0x6033884
                r5 = r5.wrapping_add(r2); // 06033710 add r2,r5
                hand_over!(); cpu.call(to, &[]); take_back!();
                pr = 0x06033712;
                pc = 0x06033712;
            }
            0x06033712 => {
                r4 = cpu.read32(r15); r15 = r15.wrapping_add(4); // 06033712 mov.l @r15+,r4
                r5 = cpu.read32(r15); r15 = r15.wrapping_add(4); // 06033714 mov.l @r15+,r5
                r7 = cpu.read32(r15); r15 = r15.wrapping_add(4); // 06033716 mov.l @r15+,r7
                pc = 0x06033718;
            }
            0x06033718 => {
                let to = 0x06033884; // 06033718 bsr 0x6033884
                // nop // 0603371a nop 
                hand_over!(); cpu.call(to, &[]); take_back!();
                pr = 0x0603371c;
                pc = 0x0603371c;
            }
            0x0603371c => {
                cpu.write32(r8.wrapping_add(4), r4); // 0603371c mov.l r4,@(4,r8)
                r2 = cpu.read32(r8.wrapping_add(16)); // 0603371e mov.l @(16,r8),r2
                r3 = cpu.read32(r8.wrapping_add(12)); // 06033720 mov.l @(12,r8),r3
                r0 = cpu.read8(r8.wrapping_add(0)) as i8 as i32 as u32; // 06033722 mov.b @(0,r8),r0
                r2 = r2.wrapping_add(r7); // 06033724 add r7,r2
                t = r3 == r2; // 06033726 cmp/eq r2,r3
                let take = !t; // 06033728 bf/s 0x603372e
                r0 = r0.wrapping_add(4i32 as u32); // 0603372a add #4,r0
                pc = if take { 0x0603372e } else { 0x0603372c };
            }
            0x0603372c => {
                r0 = r0.wrapping_add(64i32 as u32); // 0603372c add #64,r0
                pc = 0x0603372e;
            }
            0x0603372e => {
                cpu.write32(r8.wrapping_add(16), r2); // 0603372e mov.l r2,@(16,r8)
                cpu.write8(r8.wrapping_add(0), r0 as u8); // 06033730 mov.b r0,@(0,r8)
                pc = 0x06033732;
            }
            0x06033732 => {
                r9 = r9.wrapping_sub(1); t = r9 == 0; // 06033732 dt r9
                let take = !t; // 06033734 bf/s 0x6033662
                r8 = r8.wrapping_add(20i32 as u32); // 06033736 add #20,r8
                pc = if take { 0x06033662 } else { 0x06033738 };
            }
            0x06033738 => {
                r9 = cpu.read32(r15); r15 = r15.wrapping_add(4); // 06033738 mov.l @r15+,r9
                pr = cpu.read32(r15); r15 = r15.wrapping_add(4); // 0603373a lds.l @r15+,pr
                // 0603373c rts
                r8 = cpu.read32(r15); r15 = r15.wrapping_add(4); // 0603373e mov.l @r15+,r8
                if pr == entry_pr { break; }
                pc = pr;
            }
            0x06033740 => {
                r0 = cpu.read8(r8.wrapping_add(1)) as i8 as i32 as u32; // 06033740 mov.b @(1,r8),r0
                r2 = 0x25a007a0; // 06033742 mov.l 0x60339ec,r2
                r3 = r0; // 06033744 mov r0,r3
                r0 >>= 2; // 06033746 shlr2 r0
                t = r0 & 1 != 0; r0 >>= 1; // 06033748 shlr r0
                r0 &= 14; // 0603374a and #14,r0
                r2 = cpu.read8(r0.wrapping_add(r2)) as i8 as i32 as u32; // 0603374c mov.b @(r0,r2),r2
                r0 = 0x060339bc; // 0603374e mova 0x60339bc,r0
                t = r2 & r2 == 0; // 06033750 tst r2,r2
                let take = t; // 06033752 bt/s 0x6033732
                r7 = 2i32 as u32; // 06033754 mov #2,r7
                pc = if take { 0x06033732 } else { 0x06033756 };
            }
            0x06033756 => {
                r1 = cpu.read16(r0.wrapping_add(r1)) as i16 as i32 as u32; // 06033756 mov.w @(r0,r1),r1
                r5 = cpu.read32(r8.wrapping_add(8)); // 06033758 mov.l @(8,r8),r5
                r4 = cpu.read32(r8.wrapping_add(16)); // 0603375a mov.l @(16,r8),r4
                r6 = cpu.read32(r8.wrapping_add(12)); // 0603375c mov.l @(12,r8),r6
                r0 = r3; // 0603375e mov r3,r0
                r3 = r6; // 06033760 mov r6,r3
                t = r0 & 128 == 0; // 06033762 tst #128,r0
                let take = t; // 06033764 bt/s 0x603376e
                r2 = 32i32 as u32; // 06033766 mov #32,r2
                pc = if take { 0x0603376e } else { 0x06033768 };
            }
            0x06033768 => {
                t = r7 >> 31 != 0; r7 <<= 1; // 06033768 shll r7
                t = r2 >> 31 != 0; r2 <<= 1; // 0603376a shll r2
                t = r1 >> 31 != 0; r1 <<= 1; // 0603376c shll r1
                pc = 0x0603376e;
            }
            0x0603376e => {
                r7 <<= 8; // 0603376e shll8 r7
                r6 = r6.wrapping_sub(r4); // 06033770 sub r4,r6
                t = r6 >= r7; // 06033772 cmp/hs r7,r6
                let take = t; // 06033774 bt/s 0x603379e
                r4 = cpu.read32(r8.wrapping_add(4)); // 06033776 mov.l @(4,r8),r4
                pc = if take { 0x0603379e } else { 0x06033778 };
            }
            0x06033778 => {
                t = r0 & 2 == 0; // 06033778 tst #2,r0
                let take = t; // 0603377a bt/s 0x6033792
                r5 = r5.wrapping_add(r1); // 0603377c add r1,r5
                pc = if take { 0x06033792 } else { 0x0603377e };
            }
            0x0603377e => {
                r15 = r15.wrapping_sub(4); cpu.write32(r15, r7); // 0603377e mov.l r7,@-r15
                r15 = r15.wrapping_sub(4); cpu.write32(r15, r5); // 06033780 mov.l r5,@-r15
                r15 = r15.wrapping_sub(4); cpu.write32(r15, r4); // 06033782 mov.l r4,@-r15
                r2 <<= 8; // 06033784 shll8 r2
                r4 = r4.wrapping_add(r3); // 06033786 add r3,r4
                let to = 0x06033884; // 06033788 bsr 0x6033884
                r5 = r5.wrapping_add(r2); // 0603378a add r2,r5
                hand_over!(); cpu.call(to, &[]); take_back!();
                pr = 0x0603378c;
                pc = 0x0603378c;
            }
            0x0603378c => {
                r4 = cpu.read32(r15); r15 = r15.wrapping_add(4); // 0603378c mov.l @r15+,r4
                r5 = cpu.read32(r15); r15 = r15.wrapping_add(4); // 0603378e mov.l @r15+,r5
                r7 = cpu.read32(r15); r15 = r15.wrapping_add(4); // 06033790 mov.l @r15+,r7
                pc = 0x06033792;
            }
            0x06033792 => {
                let to = 0x06033884; // 06033792 bsr 0x6033884
                // nop // 06033794 nop 
                hand_over!(); cpu.call(to, &[]); take_back!();
                pr = 0x06033796;
                pc = 0x06033796;
            }
            0x06033796 => {
                r2 = cpu.read32(r8.wrapping_add(16)); // 06033796 mov.l @(16,r8),r2
                r0 = 68i32 as u32; // 06033798 mov #68,r0
                // 0603379a bra 0x603372e
                r2 = r2.wrapping_add(r7); // 0603379c add r7,r2
                pc = 0x0603372e;
            }
            0x0603379e => {
                t = r0 & 2 == 0; // 0603379e tst #2,r0
                let take = t; // 060337a0 bt/s 0x60337b8
                r5 = r5.wrapping_add(r1); // 060337a2 add r1,r5
                pc = if take { 0x060337b8 } else { 0x060337a4 };
            }
            0x060337a4 => {
                r15 = r15.wrapping_sub(4); cpu.write32(r15, r7); // 060337a4 mov.l r7,@-r15
                r15 = r15.wrapping_sub(4); cpu.write32(r15, r5); // 060337a6 mov.l r5,@-r15
                r15 = r15.wrapping_sub(4); cpu.write32(r15, r4); // 060337a8 mov.l r4,@-r15
                r2 <<= 8; // 060337aa shll8 r2
                r4 = r4.wrapping_add(r3); // 060337ac add r3,r4
                let to = 0x06033884; // 060337ae bsr 0x6033884
                r5 = r5.wrapping_add(r2); // 060337b0 add r2,r5
                hand_over!(); cpu.call(to, &[]); take_back!();
                pr = 0x060337b2;
                pc = 0x060337b2;
            }
            0x060337b2 => {
                r4 = cpu.read32(r15); r15 = r15.wrapping_add(4); // 060337b2 mov.l @r15+,r4
                r5 = cpu.read32(r15); r15 = r15.wrapping_add(4); // 060337b4 mov.l @r15+,r5
                r7 = cpu.read32(r15); r15 = r15.wrapping_add(4); // 060337b6 mov.l @r15+,r7
                pc = 0x060337b8;
            }
            0x060337b8 => {
                let to = 0x06033884; // 060337b8 bsr 0x6033884
                // nop // 060337ba nop 
                hand_over!(); cpu.call(to, &[]); take_back!();
                pr = 0x060337bc;
                pc = 0x060337bc;
            }
            0x060337bc => {
                r2 = cpu.read32(r8.wrapping_add(16)); // 060337bc mov.l @(16,r8),r2
                r3 = cpu.read32(r8.wrapping_add(12)); // 060337be mov.l @(12,r8),r3
                r2 = r2.wrapping_add(r7); // 060337c0 add r7,r2
                t = r3 == r2; // 060337c2 cmp/eq r2,r3
                let take = t; // 060337c4 bt/s 0x603372e
                r0 = 68i32 as u32; // 060337c6 mov #68,r0
                pc = if take { 0x0603372e } else { 0x060337c8 };
            }
            0x060337c8 => {
                r0 = cpu.read16(r8.wrapping_add(0)) as i16 as i32 as u32; // 060337c8 mov.w @(0,r8),r0
                r7 = 1i32 as u32; // 060337ca mov #1,r7
                r1 = (r0 & 0xffff_0000) | ((r0 & 0xff) << 8) | ((r0 >> 8) & 0xff); // 060337cc swap.b r0,r1
                r7 &= r1; // 060337ce and r1,r7
                t = r0 & 128 == 0; // 060337d0 tst #128,r0
                let take = !t; // 060337d2 bf/s 0x60337da
                r1 = 2i32 as u32; // 060337d4 mov #2,r1
                pc = if take { 0x060337da } else { 0x060337d6 };
            }
            0x060337d6 => {
                r1 = 1i32 as u32; // 060337d6 mov #1,r1
                r1 = r1.wrapping_sub(r7); // 060337d8 sub r7,r1
                pc = 0x060337da;
            }
            0x060337da => {
                r4 = r4.wrapping_sub(r1); // 060337da sub r1,r4
                r2 = r2.wrapping_sub(r1); // 060337dc sub r1,r2
                cpu.write32(r8.wrapping_add(4), r4); // 060337de mov.l r4,@(4,r8)
                r7 = r7.wrapping_add(4i32 as u32); // 060337e0 add #4,r7
                // 060337e2 bra 0x603372e
                r0 = r7; // 060337e4 mov r7,r0
                pc = 0x0603372e;
            }
            0x060337e6 => {
                r0 = cpu.read8(r8.wrapping_add(1)) as i8 as i32 as u32; // 060337e6 mov.b @(1,r8),r0
                r2 = 0x25a007a0; // 060337e8 mov.l 0x60339ec,r2
                r3 = r0; // 060337ea mov r0,r3
                r0 >>= 2; // 060337ec shlr2 r0
                t = r0 & 1 != 0; r0 >>= 1; // 060337ee shlr r0
                r0 &= 14; // 060337f0 and #14,r0
                r2 = cpu.read8(r0.wrapping_add(r2)) as i8 as i32 as u32; // 060337f2 mov.b @(r0,r2),r2
                r0 = 0x060339bc; // 060337f4 mova 0x60339bc,r0
                t = r2 & r2 == 0; // 060337f6 tst r2,r2
                let take = t; // 060337f8 bt/s 0x6033732
                r0 = r0.wrapping_add(2i32 as u32); // 060337fa add #2,r0
                pc = if take { 0x06033732 } else { 0x060337fc };
            }
            0x060337fc => {
                r2 = cpu.read16(r0.wrapping_add(r1)) as i16 as i32 as u32; // 060337fc mov.w @(r0,r1),r2
                r0 = cpu.read16(r8.wrapping_add(2)) as i16 as i32 as u32; // 060337fe mov.w @(2,r8),r0
                r6 = 2i32 as u32; // 06033800 mov #2,r6
                r0 &= r2; // 06033802 and r2,r0
                t = r0 & r0 == 0; // 06033804 tst r0,r0
                let take = !t; // 06033806 bf/s 0x6033836
                r2 = 68i32 as u32; // 06033808 mov #68,r2
                pc = if take { 0x06033836 } else { 0x0603380a };
            }
            0x0603380a => {
                // 0603380a bra 0x6033836
                r2 = 112i32 as u32; // 0603380c mov #112,r2
                pc = 0x06033836;
            }
            0x0603380e => {
                r0 = cpu.read8(r8.wrapping_add(1)) as i8 as i32 as u32; // 0603380e mov.b @(1,r8),r0
                r2 = 0x25a007a0; // 06033810 mov.l 0x60339ec,r2
                r3 = r0; // 06033812 mov r0,r3
                r0 >>= 2; // 06033814 shlr2 r0
                t = r0 & 1 != 0; r0 >>= 1; // 06033816 shlr r0
                r0 &= 14; // 06033818 and #14,r0
                r2 = cpu.read8(r0.wrapping_add(r2)) as i8 as i32 as u32; // 0603381a mov.b @(r0,r2),r2
                t = r2 & r2 == 0; // 0603381c tst r2,r2
                pc = if !t { 0x06033732 } else { 0x06033820 }; // 0603381e bf 0x6033732
            }
            0x06033820 => {
                r0 = 0x060339bc; // 06033820 mova 0x60339bc,r0
                r0 = r0.wrapping_add(2i32 as u32); // 06033822 add #2,r0
                r2 = cpu.read16(r0.wrapping_add(r1)) as i16 as i32 as u32; // 06033824 mov.w @(r0,r1),r2
                r0 = cpu.read16(r8.wrapping_add(2)) as i16 as i32 as u32; // 06033826 mov.w @(2,r8),r0
                r6 = 3i32 as u32; // 06033828 mov #3,r6
                r0 &= r2; // 0603382a and r2,r0
                r2 = r1; // 0603382c mov r1,r2
                t = r0 & r0 == 0; // 0603382e tst r0,r0
                let take = !t; // 06033830 bf/s 0x6033836
                r2 = r2.wrapping_add(68i32 as u32); // 06033832 add #68,r2
                pc = if take { 0x06033836 } else { 0x06033834 };
            }
            0x06033834 => {
                r2 = 112i32 as u32; // 06033834 mov #112,r2
                pc = 0x06033836;
            }
            0x06033836 => {
                cpu.write8(r8, r2 as u8); // 06033836 mov.b r2,@r8
                cpu.write16(r8.wrapping_add(2), r0 as u16); // 06033838 mov.w r0,@(2,r8)
                r0 = 0x060339bc; // 0603383a mova 0x60339bc,r0
                r7 = cpu.read16(r0.wrapping_add(r1)) as i16 as i32 as u32; // 0603383c mov.w @(r0,r1),r7
                r5 = cpu.read32(r8.wrapping_add(8)); // 0603383e mov.l @(8,r8),r5
                r0 = 0x060339f0; // 06033840 mova 0x60339f0,r0
                let to = 0x060338da; // 06033842 bsr 0x60338da
                r4 = r0; // 06033844 mov r0,r4
                hand_over!(); cpu.call(to, &[]); take_back!();
                pr = 0x06033846;
                pc = 0x06033846;
            }
            0x06033846 => {
                r0 = cpu.read8(r8.wrapping_add(1)) as i8 as i32 as u32; // 06033846 mov.b @(1,r8),r0
                cpu.write32(r3.wrapping_add(0), r4); // 06033848 mov.l r4,@(0,r3)
                t = r0 & 128 == 0; // 0603384a tst #128,r0
                let take = t; // 0603384c bt/s 0x6033856
                r1 = 32i32 as u32; // 0603384e mov #32,r1
                pc = if take { 0x06033856 } else { 0x06033850 };
            }
            0x06033850 => {
                t = r6 >> 31 != 0; r6 <<= 1; // 06033850 shll r6
                t = r7 >> 31 != 0; r7 <<= 1; // 06033852 shll r7
                t = r1 >> 31 != 0; r1 <<= 1; // 06033854 shll r1
                pc = 0x06033856;
            }
            0x06033856 => {
                r6 <<= 8; // 06033856 shll8 r6
                cpu.write32(r3.wrapping_add(8), r6); // 06033858 mov.l r6,@(8,r3)
                r4 = 1i32 as u32; // 0603385a mov #1,r4
                cpu.write32(r3.wrapping_add(12), r4); // 0603385c mov.l r4,@(12,r3)
                r4 = 7i32 as u32; // 0603385e mov #7,r4
                cpu.write32(r3.wrapping_add(20), r4); // 06033860 mov.l r4,@(20,r3)
                t = r0 & 2 == 0; // 06033862 tst #2,r0
                let take = t; // 06033864 bt/s 0x6033878
                r5 = r5.wrapping_add(r7); // 06033866 add r7,r5
                pc = if take { 0x06033878 } else { 0x06033868 };
            }
            0x06033868 => {
                r1 <<= 8; // 06033868 shll8 r1
                r1 = r1.wrapping_add(r5); // 0603386a add r5,r1
                r0 = 8i32 as u32; // 0603386c mov #8,r0
                cpu.write8(w.wrapping_add(189), r0 as u8); // 0603386e mov.b r0,@(189,gbr)
                r0 = 0x00000101; // 06033870 mov.w 0x6033934,r0
                cpu.write32(r3.wrapping_add(4), r1); // 06033872 mov.l r1,@(4,r3)
                let to = 0x060338da; // 06033874 bsr 0x60338da
                cpu.write32(r3.wrapping_add(16), r0); // 06033876 mov.l r0,@(16,r3)
                hand_over!(); cpu.call(to, &[]); take_back!();
                pr = 0x06033878;
                pc = 0x06033878;
            }
            0x06033878 => {
                r0 = 8i32 as u32; // 06033878 mov #8,r0
                cpu.write8(w.wrapping_add(189), r0 as u8); // 0603387a mov.b r0,@(189,gbr)
                r0 = 0x00000101; // 0603387c mov.w 0x6033934,r0
                cpu.write32(r3.wrapping_add(4), r5); // 0603387e mov.l r5,@(4,r3)
                // 06033880 bra 0x6033732
                cpu.write32(r3.wrapping_add(16), r0); // 06033882 mov.l r0,@(16,r3)
                pc = 0x06033732;
            }
            _ => unreachable!("{pc:#010x}"),
        }
    }
    hand_over!();
    cpu.leave_r14(r14);
}


/// A stream's stretch copied (pcm_streams's: by the SCU's level-2 DMA, its wrap at the buffer's
/// end done as a second copy, or by the CPU's channel 1). Not seen run.
pub fn pcm_copy(cpu: &mut Cpu) {
    pcm_copy_blocks(cpu);
}

#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
fn pcm_copy_blocks(cpu: &mut Cpu) {
    let w = cpu.gbr();
    let [mut r0, mut r1, mut r2, mut r3, mut r4, mut r5, mut r6, mut r7, mut r8, mut r9, mut r10, mut r11, mut r12, mut r13, _, mut r15] = cpu.sh2.r;
    let mut r14 = cpu.caller_r14;
    let mut mac = ((cpu.sh2.mach as u64) << 32 | cpu.sh2.macl as u64) as i64;
    let mut t = cpu.sh2.sr & 1 != 0;
    let entry_pr = cpu.sh2.pr;
    let mut pr = entry_pr;
    macro_rules! hand_over { () => { cpu.sh2.r = [r0, r1, r2, r3, r4, r5, r6, r7, r8, r9, r10, r11, r12, r13, r14, r15]; cpu.leave_mac(mac); cpu.leave_t(t); } }
    macro_rules! take_back { () => { [r0, r1, r2, r3, r4, r5, r6, r7, r8, r9, r10, r11, r12, r13, r14, r15] = cpu.sh2.r; mac = ((cpu.sh2.mach as u64) << 32 | cpu.sh2.macl as u64) as i64; t = cpu.sh2.sr & 1 != 0; } }
    let mut pc: u32 = 0x06033884;
    loop {
        match pc {
            0x06033884 => {
                r15 = r15.wrapping_sub(4); cpu.write32(r15, pr); // 06033884 sts.l pr,@-r15
                let to = 0x060338da; // 06033886 bsr 0x60338da
                // nop // 06033888 nop 
                hand_over!(); cpu.call(to, &[]); take_back!();
                pr = 0x0603388a;
                pc = 0x0603388a;
            }
            0x0603388a => {
                r0 = 0i32 as u32; // 0603388a mov #0,r0
                cpu.write32(r3.wrapping_add(16), r0); // 0603388c mov.l r0,@(16,r3)
                t = r6 >= r7; // 0603388e cmp/hs r7,r6
                let take = t; // 06033890 bt/s 0x60338b8
                r1 = 7i32 as u32; // 06033892 mov #7,r1
                pc = if take { 0x060338b8 } else { 0x06033894 };
            }
            0x06033894 => {
                r2 = r5; // 06033894 mov r5,r2
                r0 = 0x060339f0; // 06033896 mova 0x60339f0,r0
                r2 = r2.wrapping_add(r6); // 06033898 add r6,r2
                r7 = r7.wrapping_sub(r6); // 0603389a sub r6,r7
                cpu.write32(r3.wrapping_add(0), r0); // 0603389c mov.l r0,@(0,r3)
                cpu.write32(r3.wrapping_add(4), r2); // 0603389e mov.l r2,@(4,r3)
                cpu.write32(r3.wrapping_add(8), r7); // 060338a0 mov.l r7,@(8,r3)
                r0 = 1i32 as u32; // 060338a2 mov #1,r0
                cpu.write32(r3.wrapping_add(12), r0); // 060338a4 mov.l r0,@(12,r3)
                r0 = 8i32 as u32; // 060338a6 mov #8,r0
                cpu.write8(w.wrapping_add(189), r0 as u8); // 060338a8 mov.b r0,@(189,gbr)
                r0 = 0x00000101; // 060338aa mov.w 0x6033934,r0
                cpu.write32(r3.wrapping_add(20), r1); // 060338ac mov.l r1,@(20,r3)
                let to = 0x060338da; // 060338ae bsr 0x60338da
                cpu.write32(r3.wrapping_add(16), r0); // 060338b0 mov.l r0,@(16,r3)
                hand_over!(); cpu.call(to, &[]); take_back!();
                pr = 0x060338b2;
                pc = 0x060338b2;
            }
            0x060338b2 => {
                r7 = r6; // 060338b2 mov r6,r7
                r0 = 0i32 as u32; // 060338b4 mov #0,r0
                cpu.write32(r3.wrapping_add(16), r0); // 060338b6 mov.l r0,@(16,r3)
                pc = 0x060338b8;
            }
            0x060338b8 => {
                r0 = cpu.read8(r8.wrapping_add(0)) as i8 as i32 as u32; // 060338b8 mov.b @(0,r8),r0
                t = r7 & r7 == 0; // 060338ba tst r7,r7
                let take = t; // 060338bc bt/s 0x60338d6
                pr = cpu.read32(r15); r15 = r15.wrapping_add(4); // 060338be lds.l @r15+,pr
                pc = if take { 0x060338d6 } else { 0x060338c0 };
            }
            0x060338c0 => {
                t = r0 & 1 == 0; // 060338c0 tst #1,r0
                pc = if !t { 0x060338f0 } else { 0x060338c4 }; // 060338c2 bf 0x60338f0
            }
            0x060338c4 => {
                r2 = 0x00000101; // 060338c4 mov.w 0x6033934,r2
                cpu.write32(r3.wrapping_add(0), r4); // 060338c6 mov.l r4,@(0,r3)
                cpu.write32(r3.wrapping_add(4), r5); // 060338c8 mov.l r5,@(4,r3)
                cpu.write32(r3.wrapping_add(8), r7); // 060338ca mov.l r7,@(8,r3)
                cpu.write32(r3.wrapping_add(12), r2); // 060338cc mov.l r2,@(12,r3)
                cpu.write32(r3.wrapping_add(20), r1); // 060338ce mov.l r1,@(20,r3)
                cpu.write32(r3.wrapping_add(16), r2); // 060338d0 mov.l r2,@(16,r3)
                r0 = 8i32 as u32; // 060338d2 mov #8,r0
                cpu.write8(w.wrapping_add(189), r0 as u8); // 060338d4 mov.b r0,@(189,gbr)
                pc = 0x060338d6;
            }
            0x060338d6 => {
                // 060338d6 rts
                r4 = r4.wrapping_add(r7); // 060338d8 add r7,r4
                if pr == entry_pr { break; }
                pc = pr;
            }
            0x060338f0 => {
                r3 = -112i32 as u32; // 060338f0 mov #-112,r3
                pc = 0x060338f2;
            }
            0x060338f2 => {
                r0 = cpu.read32(r3.wrapping_add(12)); // 060338f2 mov.l @(12,r3),r0
                r0 &= 3; // 060338f4 and #3,r0
                t = r0 == 1i32 as u32; // 060338f6 cmp/eq #1,r0
                let take = t; // 060338f8 bt/s 0x60338f2
                r0 = r7; // 060338fa mov r7,r0
                pc = if take { 0x060338f2 } else { 0x060338fc };
                if take { cpu.wait(0x060338f2, 5); }
            }
            0x060338fc => {
                r0 |= r4; // 060338fc or r4,r0
                r2 = r7; // 060338fe mov r7,r2
                t = r2 & 1 != 0; r2 >>= 1; // 06033900 shlr r2
                t = r0 & 2 == 0; // 06033902 tst #2,r0
                let take = !t; // 06033904 bf/s 0x603390c
                r1 = 4i32 as u32; // 06033906 mov #4,r1
                pc = if take { 0x0603390c } else { 0x06033908 };
            }
            0x06033908 => {
                t = r2 & 1 != 0; r2 >>= 1; // 06033908 shlr r2
                r1 = 8i32 as u32; // 0603390a mov #8,r1
                pc = 0x0603390c;
            }
            0x0603390c => {
                r0 = cpu.read32(r3.wrapping_add(32)); // 0603390c mov.l @(32,r3),r0
                r0 = 0i32 as u32; // 0603390e mov #0,r0
                cpu.write32(r3.wrapping_add(12), r0); // 06033910 mov.l r0,@(12,r3)
                r3 = -100i32 as u32; // 06033912 mov #-100,r3
                r3 <<= 2; // 06033914 shll2 r3
                cpu.write8(r3.wrapping_add(2), r0 as u8); // 06033916 mov.b r0,@(2,r3)
                r3 = -112i32 as u32; // 06033918 mov #-112,r3
                cpu.write32(r3.wrapping_add(0), r4); // 0603391a mov.l r4,@(0,r3)
                cpu.write32(r3.wrapping_add(4), r5); // 0603391c mov.l r5,@(4,r3)
                cpu.write32(r3.wrapping_add(8), r2); // 0603391e mov.l r2,@(8,r3)
                r0 = 0x00005201; // 06033920 mov.w 0x6033936,r0
                r1 <<= 8; // 06033922 shll8 r1
                r0 |= r1; // 06033924 or r1,r0
                cpu.write32(r3.wrapping_add(12), r0); // 06033926 mov.l r0,@(12,r3)
                r0 = 8i32 as u32; // 06033928 mov #8,r0
                cpu.write8(w.wrapping_add(186), r0 as u8); // 0603392a mov.b r0,@(186,gbr)
                r0 = 9i32 as u32; // 0603392c mov #9,r0
                cpu.write32(r3.wrapping_add(32), r0); // 0603392e mov.l r0,@(32,r3)
                // 06033930 rts
                r4 = r4.wrapping_add(r7); // 06033932 add r7,r4
                if pr == entry_pr { break; }
                pc = pr;
            }
            _ => unreachable!("{pc:#010x}"),
        }
    }
    hand_over!();
    cpu.leave_r14(r14);
}


/// The SCU's level-2 DMA waited for (its done flag; looked at every 48 counts). Not seen run.
pub fn pcm_wait(cpu: &mut Cpu) {
    pcm_command_blocks(cpu);
}

#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
fn pcm_command_blocks(cpu: &mut Cpu) {
    let w = cpu.gbr();
    let [mut r0, mut r1, mut r2, mut r3, mut r4, mut r5, mut r6, mut r7, mut r8, mut r9, mut r10, mut r11, mut r12, mut r13, _, mut r15] = cpu.sh2.r;
    let mut r14 = cpu.caller_r14;
    let mut mac = ((cpu.sh2.mach as u64) << 32 | cpu.sh2.macl as u64) as i64;
    let mut t = cpu.sh2.sr & 1 != 0;
    let entry_pr = cpu.sh2.pr;
    let mut pr = entry_pr;
    macro_rules! hand_over { () => { cpu.sh2.r = [r0, r1, r2, r3, r4, r5, r6, r7, r8, r9, r10, r11, r12, r13, r14, r15]; cpu.leave_mac(mac); cpu.leave_t(t); } }
    macro_rules! take_back { () => { [r0, r1, r2, r3, r4, r5, r6, r7, r8, r9, r10, r11, r12, r13, r14, r15] = cpu.sh2.r; mac = ((cpu.sh2.mach as u64) << 32 | cpu.sh2.macl as u64) as i64; t = cpu.sh2.sr & 1 != 0; } }
    let mut pc: u32 = 0x060338da;
    loop {
        match pc {
            0x060338da => {
                r3 = 0x25fe0040; // 060338da mov.l 0x60339f4,r3
                r0 = cpu.read8(w.wrapping_add(189)) as i8 as i32 as u32; // 060338dc mov.b @(189,gbr),r0
                pc = 0x060338de;
            }
            0x060338de => {
                t = r0 & 4 == 0; // 060338de tst #4,r0
                let take = !t; // 060338e0 bf/s 0x60338ec
                r0 = 48i32 as u32; // 060338e2 mov #48,r0
                pc = if take { 0x060338ec } else { 0x060338e4 };
            }
            0x060338e4 => {
                r0 = r0.wrapping_sub(1); t = r0 == 0; // 060338e4 dt r0
                pc = if !t { 0x060338e4 } else { 0x060338e8 }; // 060338e6 bf 0x60338e4
                if pc == 0x060338e4 { cpu.wait(0x060338e4, 3); }
            }
            0x060338e8 => {
                // 060338e8 bra 0x60338de
                r0 = cpu.read8(w.wrapping_add(189)) as i8 as i32 as u32; // 060338ea mov.b @(189,gbr),r0
                pc = 0x060338de;
                cpu.wait(0x060338de, 7);
            }
            0x060338ec => {
                // 060338ec rts
                // nop // 060338ee nop 
                if pr == entry_pr { break; }
                pc = pr;
            }
            _ => unreachable!("{pc:#010x}"),
        }
    }
    hand_over!();
    cpu.leave_r14(r14);
}

/// SGL's VDP2 and VRAM set-up (slInitSystem's, with the TV mode `mode` in r4): the screen mode's
/// low bits the mode's, the screen's halves from SGL's table of them; its default VDP2 register
/// image and the two rotation parameter tables (centred on the screen) copied in; the TV mode's
/// low byte in the image, the sprite control for mode bit 3; the image sent to VDP2; VDP2's VRAM
/// cleared (a fill by DMA); SGL's fonts made into 8-bit cells (at 0x25e60000: 128 characters,
/// then 24, and 3 at 0x25e63f40) and copied to VRAM's start; the planes' cell and map places; the
/// default colours; the display on in the image (sent at the next VBlank). Each step waits for DMA
/// channel 0 as the SH-2 code does, and the code takes its time block by block (`Cpu::spend`): the
/// slave's start (slInitSystem's SSHON just before, not waited for) relies on how long it takes,
/// about 37 lines at the normal clock; the signal after it must find the slave reset and polling.
const VDP2_DEFAULTS: u32 = 0x0603_58a0;
const VDP2_IMAGE_COPY: u32 = 0x90;
const ROTATION_DEFAULTS: u32 = 0x0603_5840;
const ROTATION_TABLE_A_AT: u32 = 0x060f_fe1c;
const ROTATION_LONGS: u32 = 48;
const ROTATION_B: u32 = 104;
const ROTATION_VRAM: u32 = 0x25e3_ff00;
const ROTATION_SOURCE: u32 = 0x1e0;
const ROTATION_PLACE: u32 = 0x214;
const SPRITE_CONTROL: u32 = 0x1a0;
const SPRITE_8_BIT: u16 = 12;
const VDP2_REGISTERS_AT: u32 = 0x25f8_0000;
const VRAM_FILL: u32 = 0x0603_5794;
const VDP2_VRAM_START: u32 = 0x25e0_0000;
const VDP2_VRAM_LONGS: u32 = 0x4_0000;
const FILL_WORDS: u32 = 0x4601;
const FONT: u32 = 0x0603_59f8;
const FONT_CHARACTERS: u32 = 128;
const FONT_4BIT: u32 = 0x0603_5e10;
const FONT_4BIT_CHARACTERS: u32 = 24;
const FONT_MORE: u32 = 0x0603_5df8;
const FONT_MORE_CHARACTERS: u32 = 3;
const FONT_MORE_AT: u32 = 0x25e6_3f40;
const FONT_CELLS_AT: u32 = 0x25e6_0000;
const FONT_COPY_WORDS: u32 = 0x980;
const COPY_FROM_FIXED: u32 = 0x5a01;
const FONT_PLACES: [u32; 2] = [0x1e4, 0x1e8];
const VRAM_START_PLACES: [u32; 2] = [0x1f4, 0x1f8];
const PLANE_PLACES: [(u32, u32); 4] = [(0x1fc, 0x25e7_6000), (0x200, 0x25e7_8000), (0x20c, 0x25e4_0000), (0x210, 0x25e6_0000)];
const PLANE_CLEARED: [u32; 4] = [0x2ec, 0x2f0, 0x2f4, 0x2f8];
const BYTE_21: u32 = 0x21;
const VRAM_LAST_WORD: u32 = 0x25e3_fffe;
const TVMD_IMAGE: u32 = 0xc0;
const DISPLAY_ON: u32 = 0xffff_8000;
const DMA0_COPY: u32 = 0x0603_5724;
const ROTATION_TABLE_INIT: u32 = 0x0603_56c0;
const FONT_CELLS: u32 = 0x0603_5660;
const FONT_CELLS_WIDE: u32 = 0x0603_5690;
const DEFAULT_COLOURS: u32 = 0x0603_59c0;

/// Waits for DMA channel 0 (its control's enable without its end), each pass the SH-2 loop's time
/// (`at`, 4 instructions).
fn dma0_wait_timed(cpu: &mut Cpu, at: u32) {
    loop {
        let busy = cpu.read32(CHCR0) & 3 == 1;
        cpu.spend(at, 4);
        if !busy {
            break;
        }
    }
}

/// DMA channel 0 started: `n` units from `from` to `to` by `control` (after the operation register
/// read and the channel stopped, as SGL does).
fn dma0_start(cpu: &mut Cpu, from: u32, to: u32, n: u32, control: u32) {
    cpu.read32(DMAOR);
    cpu.write32(CHCR0, 0);
    cpu.write8(DRCR0, 0);
    cpu.write32(SAR0, from);
    cpu.write32(DAR0, to);
    cpu.write32(TCR0, n);
    cpu.write32(CHCR0, control);
    cpu.write32(DMAOR, DMA_ON);
}

pub fn vdp_init(cpu: &mut Cpu) {
    let w = cpu.gbr();
    let mode = cpu.reg(4) & 31;
    let screen = cpu.read8(w + SCREEN_MODE) as i8 as i32 as u32 & 0x80 | mode;
    cpu.write8(w + SCREEN_MODE, screen as u8);
    cpu.push(&[mode]);
    let halves = cpu.read32(SCREEN_HALVES + 4 * mode);
    cpu.write32(w + SCREEN_WIDTH, halves);
    cpu.spend(0x0603_5540, 17);
    cpu.call(DMA0_COPY, &[VDP2_DEFAULTS, VDP2_IMAGE, VDP2_IMAGE_COPY]);
    cpu.spend(0x0603_5562, 5);
    cpu.call(ROTATION_TABLE_INIT, &[ROTATION_DEFAULTS, ROTATION_TABLE_A_AT, ROTATION_LONGS]);
    cpu.spend(0x0603_556c, 2);
    cpu.call(ROTATION_TABLE_INIT, &[ROTATION_DEFAULTS, ROTATION_TABLE_A_AT + ROTATION_B, ROTATION_LONGS]);
    cpu.write32(w + ROTATION_PLACE, ROTATION_VRAM);
    cpu.write32(w + ROTATION_SOURCE, ROTATION_TABLE_A_AT);
    cpu.pop(4);
    let tvmd = cpu.read8(TV_MODES + mode);
    cpu.write8(w + TVMD_LOW, tvmd);
    cpu.spend(0x0603_5570, 12);
    if mode & 8 != 0 {
        cpu.write16(w + SPRITE_CONTROL, SPRITE_8_BIT);
        cpu.spend(0x0603_5588, 1);
    }
    cpu.spend(0x0603_558a, 5);
    cpu.call(DMA0_COPY, &[VDP2_IMAGE, VDP2_REGISTERS_AT, VDP2_IMAGE_COPY]);
    cpu.spend(0x0603_5594, 1);
    dma0_wait_timed(cpu, 0x0603_5596);
    cpu.spend(0x0603_559e, 5);
    dma0_wait_timed(cpu, 0x0603_55a8);
    dma0_start(cpu, VRAM_FILL, VDP2_VRAM_START, VDP2_VRAM_LONGS, FILL_WORDS);
    cpu.spend(0x0603_55b0, 15);
    dma0_wait_timed(cpu, 0x0603_55ce);
    for at in FONT_PLACES {
        cpu.write32(w + at, FONT_CELLS_AT);
    }
    cpu.spend(0x0603_55d6, 9);
    cpu.call(FONT_CELLS, &[FONT, FONT_CHARACTERS, FONT_CELLS_AT, 0]);
    cpu.spend(0x0603_55e8, 4);
    let next = cpu.reg(6);
    cpu.call(FONT_CELLS_WIDE, &[FONT_4BIT, FONT_4BIT_CHARACTERS, next, 0]);
    cpu.spend(0x0603_55f0, 5);
    cpu.call(FONT_CELLS, &[FONT_MORE, FONT_MORE_CHARACTERS, FONT_MORE_AT, 0]);
    for at in VRAM_START_PLACES {
        cpu.write32(w + at, VDP2_VRAM_START);
    }
    cpu.spend(0x0603_55fa, 7);
    dma0_wait_timed(cpu, 0x0603_5608);
    dma0_start(cpu, FONT_CELLS_AT, VDP2_VRAM_START, FONT_COPY_WORDS, COPY_FROM_FIXED);
    for (at, v) in PLANE_PLACES {
        cpu.write32(w + at, v);
    }
    for at in PLANE_CLEARED {
        cpu.write32(w + at, 0);
    }
    cpu.write8(w + BYTE_21, 0);
    cpu.write16(VRAM_LAST_WORD, 0);
    cpu.leave(&[(4, FONT_CELLS_AT), (5, VDP2_VRAM_START), (6, FONT_COPY_WORDS)]);
    cpu.spend(0x0603_5610, 32);
    cpu.call(DEFAULT_COLOURS, &[]);
    let tvmd = cpu.read16(w + TVMD_IMAGE) as i16 as i32 as u32 | DISPLAY_ON;
    cpu.write16(w + TVMD_IMAGE, tvmd as u16);
    cpu.leave(&[(0, tvmd), (1, DISPLAY_ON)]);
    cpu.spend(0x0603_5650, 6);
}

/// SGL's 1-bit font (`n` characters of eight bytes from r4) as 8-bit cells at r6: each dot a
/// byte, the colour r7 plus the dot's bit. The SH-2 code's time block by block.
pub fn font_cells(cpu: &mut Cpu) {
    let [mut from, mut n, mut to, colour] = [cpu.reg(4), cpu.reg(5), cpu.reg(6), cpu.reg(7) & 0xff];
    cpu.spend(0x0603_5660, 1);
    let mut word = 0;
    loop {
        cpu.spend(0x0603_5662, 1);
        for _ in 0..8 {
            let mut bits = (cpu.read8(from) as u32) << 24;
            from += 1;
            cpu.spend(0x0603_5664, 4);
            for _ in 0..4 {
                let (a, b) = (bits >> 31, bits >> 30 & 1);
                bits <<= 2;
                word = ((a + colour) << 8) + b + colour;
                cpu.write16(to, word as u16);
                to += 2;
                cpu.spend(0x0603_566c, 12);
            }
            cpu.spend(0x0603_5684, 2);
        }
        n -= 1;
        cpu.spend(0x0603_5688, 2);
        if n == 0 {
            break;
        }
    }
    cpu.leave(&[(0, 0), (1, 0), (2, word), (3, 0), (4, from), (5, 0), (6, to), (7, colour)]);
    cpu.leave_t(true);
    cpu.spend(0x0603_568c, 2);
}

/// SGL's 4-bit font (`n` characters of 32 bytes from r4) as 8-bit cells at r6: each dot a byte,
/// the colour r7 plus the dot's nibble. The SH-2 code's time block by block.
pub fn font_cells_wide(cpu: &mut Cpu) {
    let [mut from, mut n, mut to] = [cpu.reg(4), cpu.reg(5), cpu.reg(6)];
    let colour = (cpu.reg(7) & 0xff) * 0x101;
    cpu.spend(0x0603_5690, 4);
    let (mut word, mut high) = (0, 0);
    loop {
        cpu.spend(0x0603_5698, 1);
        for _ in 0..8 {
            cpu.spend(0x0603_569a, 1);
            for _ in 0..4 {
                let byte = cpu.read8(from) as u32;
                from += 1;
                high = (byte >> 4) << 8;
                word = (byte as i8 as i32 as u32 & 15 | high).wrapping_add(colour);
                cpu.write16(to, word as u16);
                to += 2;
                cpu.spend(0x0603_569c, 12);
            }
            cpu.spend(0x0603_56b4, 2);
        }
        n -= 1;
        cpu.spend(0x0603_56b8, 2);
        if n == 0 {
            break;
        }
    }
    cpu.leave(&[(0, word), (1, 0), (2, high), (3, 0), (4, from), (5, 0), (6, to), (7, colour)]);
    cpu.leave_t(true);
    cpu.spend(0x0603_56bc, 2);
}

/// A rotation parameter table (r6 longs from r4 to r5) copied by DMA channel 0, waited for, then
/// centred on the screen: its scales (+0x34, +0x3c: 1 in both halves; +0x38, +0x40: 1), its screen
/// start (+0, +4) from half the screen's halves. The SH-2 code's time block by block.
pub fn rotation_table_init(cpu: &mut Cpu) {
    let w = cpu.gbr();
    let (from, to, n) = (cpu.reg(4), cpu.reg(5), cpu.reg(6));
    cpu.spend(0x0603_56c0, 1);
    dma0_wait_timed(cpu, 0x0603_56c2);
    dma0_start(cpu, from, to, n, COPY_WORDS);
    cpu.spend(0x0603_56ca, 15);
    dma0_wait_timed(cpu, 0x0603_56e8);
    let halves = cpu.read32(w + SCREEN_WIDTH);
    let half = halves >> 1;
    let one = 0x100;
    let ones = one << 16 | one;
    cpu.write32(to + 52, ones);
    cpu.write32(to + 60, ones);
    cpu.write16(to + 56, one as u16);
    cpu.write16(to + 64, one as u16);
    let (x, y) = (one.wrapping_sub(half.rotate_left(16)), one.wrapping_sub(half));
    let start = x << 16 | y & 0xffff;
    cpu.write32(to + 4, start << 16);
    cpu.write32(to, start >> 16 << 16);
    cpu.leave(&[(0, start << 16), (1, half), (2, start >> 16 << 16), (3, x)]);
    cpu.leave_t(halves & 1 != 0);
    cpu.spend(0x0603_56f0, 26);
}

/// r6 words from r4 to r5 on DMA channel 0, once it is free (not waited for). The SH-2 code's
/// time block by block.
pub fn dma0_copy_words(cpu: &mut Cpu) {
    let (from, to, n) = (cpu.reg(4), cpu.reg(5), cpu.reg(6));
    cpu.spend(0x0603_5724, 1);
    dma0_wait_timed(cpu, 0x0603_5726);
    dma0_start(cpu, from, to, n, COPY_WORDS);
    cpu.leave(&[(0, DMA_ON), (3, SAR0)]);
    cpu.leave_t(false);
    cpu.spend(0x0603_572e, 16);
}

/// SGL's default colours (8 longs: 16 colours) into colour RAM's start. The SH-2 code's time block
/// by block.
const COLOUR_DEFAULTS: u32 = 0x0603_59d8;
const COLOUR_RAM: u32 = 0x25f0_0000;

pub fn default_colours(cpu: &mut Cpu) {
    cpu.spend(0x0603_59c0, 3);
    let mut last = 0;
    for i in 0..8 {
        last = cpu.read32(COLOUR_DEFAULTS + 4 * i);
        cpu.write32(COLOUR_RAM + 4 * i, last);
        cpu.spend(0x0603_59c6, 5);
    }
    cpu.leave(&[(0, COLOUR_DEFAULTS + 32), (1, 0), (2, last), (3, COLOUR_RAM + 28)]);
    cpu.leave_t(true);
    cpu.spend(0x0603_59d0, 2);
}

/// slInitSystem (`mode` the TV mode, `textures` SGL's texture table, `frames` the frames a sync
/// takes): interrupts masked, the screen mode cleared; the CPUs' clock changed by the BIOS when
/// the mode's hi-res bit is not the clock's (the sound CPU restarted after); SGL's SMPC set up and
/// the reset button let through (RESENAB); the slave started; the matrices, VDP2 and VRAM set up;
/// the slave's function its command loop; the sync and the drawing set up; the slave signalled;
/// SGL's interrupt priorities to the BIOS, and the SCU's interrupts SGL takes (the VBlanks, the
/// SMPC's, the three DMA ends) unmasked; the sync count 5; the screen mode's bit 7 cleared and
/// interrupts on; the first frame, the SMPC's status asked for (INTBACK) and a frame, the pads and
/// two frames; the screen mode's bit 7 back. The SH-2 code's r8-r13 held across its calls, as the
/// callees' pushes leave them. Its code takes the SH-2 code's time block by block (`Cpu::spend`,
/// as the translations count it): the SMPC's requests and the first frames come when the SH-2
/// code's do (with no time here, a native attract mode drifts from the interpreter's by frame
/// 7200: a boss's flash and an explosion a frame apart).
const SET_INTERRUPT_MASK: u32 = 0x0603_5070;
const SCREEN_MODE_BYTE: u32 = SGL_WORK + SCREEN_MODE;
const BIOS_CLOCK: u32 = 0x0600_0324;
const BIOS_CHANGE_CLOCK: u32 = 0x0600_0320;
const SMPC_INIT: u32 = 0x0603_3dd4;
const SL_REQUEST_COMMAND: u32 = 0x0603_44c4;
const SLAVE_START: u32 = 0x0603_5090;
const INIT_MATRIX: u32 = 0x0603_5110;
const VDP_INIT: u32 = 0x0603_5540;
const SLAVE_FUNCTION_CACHED: u32 = 0x0603_5100;
const SLAVE_COMMANDS: u32 = 0x0603_6110;
const SYNCH_INIT: u32 = 0x0603_3590;
const DRAW_INIT: u32 = 0x0603_64e0;
const BIOS_CHANGE_PRIORITIES: u32 = 0x0600_0280;
const SGL_PRIORITIES: u32 = 0x0600_a444;
const BIOS_CHANGE_SCU_MASK: u32 = 0x0600_0344;
const SGL_INTERRUPTS: u32 = 0xffff_f17c;
const FIRST_SYNCH: u32 = 5;
const SYNCH_FIRST: u32 = 0x0603_4cb0;
const SL_SYNCH: u32 = 0x0603_4cca;
/// slRequestCommand's commands (SGL's numbers).
const SSHON: u32 = 1;
const SSHOFF: u32 = 2;
const SNDON: u32 = 3;
const SNDOFF: u32 = 4;
const RESENAB: u32 = 11;
const GETSTS: u32 = 13;
const GETPER: u32 = 14;

pub fn sl_init_system(cpu: &mut Cpu, mode: u32, textures: u32, frames: u32) {
    let frames = frames as i8 as i32 as u32;
    let held = [(8, mode & 0xffff), (9, frames), (12, 0), (13, textures)];
    cpu.spend(0x0600_7ef0, 16);
    cpu.holding(&held, |cpu| cpu.call(SET_INTERRUPT_MASK, &[15]));
    cpu.write8(SCREEN_MODE_BYTE, 0);
    let mode = mode & 31;
    let hires = mode >> 2 & 1;
    let mut held = [(8, mode), (9, frames), (10, SCREEN_MODE_BYTE), (12, 0), (13, textures), (11, cpu.reg(11))];
    let clock = cpu.read32(BIOS_CLOCK);
    cpu.spend(0x0600_7f10, 14);
    if clock != hires {
        let change = cpu.read32(BIOS_CHANGE_CLOCK);
        cpu.spend(0x0600_7f2c, 4);
        cpu.holding(&held, |cpu| cpu.call(change, &[hires]));
        held[3].1 = 1;
        cpu.spend(0x0600_7f34, 1);
    }
    cpu.holding(&held, |cpu| {
        cpu.spend(0x0600_7f36, 3);
        cpu.call(SMPC_INIT, &[]);
        cpu.spend(0x0600_7f3c, 4);
        cpu.call(SL_REQUEST_COMMAND, &[RESENAB, 0]);
    });
    held[5].1 = SL_REQUEST_COMMAND;
    cpu.holding(&held, |cpu| {
        cpu.spend(0x0600_7f44, 3);
        cpu.call(SLAVE_START, &[]);
        cpu.spend(0x0600_7f4a, 3);
        cpu.call(INIT_MATRIX, &[]);
        cpu.spend(0x0600_7f50, 3);
        cpu.call(VDP_INIT, &[mode]);
        cpu.write32(SLAVE_FUNCTION_CACHED, SLAVE_COMMANDS);
        cpu.spend(0x0600_7f56, 7);
        cpu.call(SYNCH_INIT, &[mode, frames]);
        cpu.spend(0x0600_7f64, 4);
        cpu.call(DRAW_INIT, &[mode, textures]);
        cpu.write16(SLAVE_SIGNAL, 0xffff);
        let priorities = cpu.read32(BIOS_CHANGE_PRIORITIES);
        cpu.spend(0x0600_7f6c, 8);
        cpu.call(priorities, &[SGL_PRIORITIES]);
        let change_mask = cpu.read32(BIOS_CHANGE_SCU_MASK);
        cpu.spend(0x0600_7f7c, 5);
        cpu.call(change_mask, &[SGL_INTERRUPTS, 0]);
    });
    cpu.write8(SGL_WORK + SYNCH_COUNT, FIRST_SYNCH as u8);
    let screen = cpu.read8(SCREEN_MODE_BYTE);
    cpu.write8(SCREEN_MODE_BYTE, screen & 0x7f);
    held[1].1 = screen as i8 as i32 as u32;
    let changed = held[3].1 == 1;
    cpu.holding(&held, |cpu| {
        cpu.spend(0x0600_7f86, 11);
        cpu.call(SET_INTERRUPT_MASK, &[0]);
        cpu.spend(0x0600_7f9c, 3);
        if changed {
            cpu.spend(0x0600_7fa2, 3);
            nested!(cpu, 0x0600_7e8c, sound_restart(cpu));
        }
        cpu.spend(0x0600_7fa8, 3);
        cpu.call(SYNCH_FIRST, &[]);
        cpu.spend(0x0600_7fae, 3);
        cpu.call(SL_REQUEST_COMMAND, &[GETSTS, 0]);
    });
    held[0].1 = SL_SYNCH;
    cpu.holding(&held, |cpu| {
        cpu.spend(0x0600_7fb4, 3);
        cpu.call(SL_SYNCH, &[]);
        cpu.spend(0x0600_7fba, 3);
        cpu.call(SL_REQUEST_COMMAND, &[GETPER, 0]);
        cpu.spend(0x0600_7fc0, 2);
        cpu.call(SL_SYNCH, &[]);
        cpu.spend(0x0600_7fc4, 2);
        cpu.call(SL_SYNCH, &[]);
    });
    let now = cpu.read8(SCREEN_MODE_BYTE);
    cpu.write8(SCREEN_MODE_BYTE, now | screen & 0x80);
    cpu.spend(0x0600_7fc8, 16);
}

/// The sound CPU restarted after a change of clock (slInitSystem's; not seen run): off (SNDOFF,
/// waited for), sound RAM 4 Mbit (SCSP's MEM4MB), the 68000's vectors all 0x400, its stack at the
/// top, and a loop at 0x400 (nop; bra back) for it to run; on again (SNDON, waited for).
const SCSP_MEMORY_MODE: u32 = 0x25b0_0400;
const SOUND_RAM: u32 = 0x25a0_0000;
const SOUND_IDLE: u32 = 0x400;
const SOUND_VECTORS_END: u32 = 0x3fc;
const SOUND_STACK: u32 = 0x0007_fffc;
const SOUND_IDLE_LOOP: u32 = 0x4e71_60fc;

pub fn sound_restart(cpu: &mut Cpu) {
    cpu.call(SL_REQUEST_COMMAND, &[SNDOFF, 1]);
    cpu.write8(SCSP_MEMORY_MODE, 2);
    nested!(cpu, 0x0600_7e70, fill_vectors(cpu, SOUND_RAM, SOUND_IDLE, SOUND_VECTORS_END));
    cpu.write32(SOUND_RAM, SOUND_STACK);
    cpu.write32(SOUND_RAM + SOUND_IDLE, SOUND_IDLE_LOOP);
    cpu.call(SL_REQUEST_COMMAND, &[SNDON, 1]);
}

/// `v` in each long from `at` while its offset under `mask` is no more than `mask` (the sound
/// CPU's vectors; not seen run).
pub fn fill_vectors(cpu: &mut Cpu, at: u32, v: u32, mask: u32) {
    let (mut at, mut off) = (at, at & mask);
    while off as i32 <= mask as i32 {
        cpu.write32(at, v);
        off = off.wrapping_add(4);
        at = at.wrapping_add(4);
    }
    cpu.leave(&[(1, off), (4, at)]);
}

/// The slave CPU started (slInitSystem's): SGL's word beside its code (0x06035104, read nowhere)
/// cleared, the slave off (SSHOFF, waited for), the BIOS's start address for it SGL's slave main,
/// and on again (SSHON, not waited for; the code jumps to the request with its own return): what
/// that request said.
const SLAVE_START_WORD: u32 = 0x0603_5104;
const BIOS_SLAVE_START: u32 = 0x0600_0250;
const SLAVE_MAIN: u32 = 0x0603_50b0;

pub fn slave_start(cpu: &mut Cpu) -> u32 {
    cpu.write32(SLAVE_START_WORD, 0);
    cpu.call(SL_REQUEST_COMMAND, &[SSHOFF, 1]);
    cpu.write32(BIOS_SLAVE_START, SLAVE_MAIN);
    let ret = cpu.pr();
    cpu.pop(4);
    cpu.call_returning(ret, SL_REQUEST_COMMAND, &[SSHON, 0])
}

/// The first frame (slInitSystem's): VBlank waited for (VDP2's TVSTAT), the sync count one below
/// the frames a sync takes, VDP1's frame buffer change (FBCR) cleared unless the screen mode's bit
/// 4 says otherwise; then slSynch, which the code runs on into (its push made first): its count.
/// Its own code takes the SH-2 code's time, as slInitSystem's does.
pub fn synch_first(cpu: &mut Cpu) -> u32 {
    cpu.spend(0x0603_4cb0, 1);
    loop {
        let status = cpu.read16(VDP2_TVSTAT);
        cpu.spend(0x0603_4cb2, 3);
        if status & 2 != 0 {
            break;
        }
    }
    let w = cpu.gbr();
    let frames = cpu.read8(w + SYNCH_FRAMES);
    cpu.write8(w + SYNCH_COUNT, frames.wrapping_sub(1));
    let mode = cpu.read8(w + SCREEN_MODE);
    cpu.spend(0x0603_4cb8, 6);
    if mode & 16 == 0 {
        cpu.write16(VDP1_FBCR, 0);
        cpu.spend(0x0603_4cc4, 3);
    }
    // inline: the code runs on into slSynch, its push made at the entry
    sl_synch(cpu)
}

/// The slave CPU's main (its start address from the BIOS on; never returns): every interrupt
/// masked (SR from -16: M, Q and the mask set), INTC's IPRA and IPRB cleared, FRT's interrupts off
/// (TIER), its cache purged on the stack SGL keeps for it; then at each signal from the master
/// (FRT's input capture, FTCSR's bit 7, cleared), the function SGL leaves it (its command loop)
/// taken and called.
const IPRA: u32 = 0xffff_fee2;
const IPRB: u32 = 0xffff_fe60;
const TIER: u32 = 0xffff_fe10;
const INPUT_CAPTURE: u8 = 0x80;
const SLAVE_SR: u32 = 0x3f0;
const SLAVE_STACK: u32 = 0x0600_89b8;
const PURGE_CACHE: u32 = 0x0603_3a6a;
const SLAVE_FUNCTION: u32 = 0x2603_5100;

pub fn slave_main(cpu: &mut Cpu) {
    cpu.sh2.sr = SLAVE_SR;
    cpu.write16(IPRA, 0);
    cpu.write16(IPRB, 0);
    cpu.write8(TIER, 1);
    cpu.sh2.r[15] = cpu.read32(SLAVE_STACK);
    cpu.call_returning(0x0603_50cc, PURGE_CACHE, &[]);
    loop {
        while cpu.read8(FTCSR) & INPUT_CAPTURE == 0 {
            cpu.wait(0x0603_50cc, 6);
        }
        let f = cpu.read32(SLAVE_FUNCTION);
        cpu.write8(FTCSR, 0);
        if f != 0 {
            cpu.write32(SLAVE_FUNCTION, 0);
            cpu.leave(&[(0, 0), (1, f), (2, SLAVE_FUNCTION), (8, TIER)]);
            cpu.call_returning(0x0603_50e2, f, &[]);
        }
    }
}

/// The slave's command loop (SGL's slave function: from the first signal on, it never returns):
/// GBR SGL's work area, the slave's place in the master's commands (+68) the command area's start
/// (cache-through); each command up to the master's end mark run, by its first word's bits 2-6 in
/// SGL's table of handlers, with r0 the second word and r4 past it (the handler moves r4 past the
/// words it takes), the place kept as it goes. An end mark behind the place (the master began the
/// area again) takes the place back to the start; when they meet, the slave waits for the master's
/// signal (FRT's input capture) and clears it.
const SLAVE_PLACE: u32 = 68;
const SLAVE_END: u32 = 0x260f_fc48;
const SLAVE_HANDLERS: u32 = 0x0603_6174;
const SLAVE_HANDLER_MASK: u32 = 124;

pub fn slave_commands(cpu: &mut Cpu) {
    cpu.sh2.gbr = SGL_WORK;
    let w = SGL_WORK;
    let mut at = cpu.read32(COMMAND_AREA) | CACHE_THROUGH;
    cpu.write32(w + SLAVE_PLACE, at);
    loop {
        let mut end = cpu.read32(SLAVE_END);
        while end != at {
            let word = cpu.read32(at);
            let handler = cpu.read32(SLAVE_HANDLERS + (word & SLAVE_HANDLER_MASK));
            let arg = cpu.read32(at + 4);
            cpu.leave(&[(0, arg), (1, handler), (2, SLAVE_END)]);
            cpu.call_returning(0x0603_6138, handler, &[at + 8]);
            at = cpu.reg(4);
            end = cpu.read32(SLAVE_END);
            cpu.write32(w + SLAVE_PLACE, at);
            if (end as i32) < at as i32 {
                at = cpu.read32(COMMAND_AREA) | CACHE_THROUGH;
                cpu.write32(w + SLAVE_PLACE, at);
                end = cpu.read32(SLAVE_END);
            }
        }
        while cpu.read8(FTCSR) & INPUT_CAPTURE == 0 {
            cpu.wait(0x0603_6152, 4);
        }
        cpu.write8(FTCSR, 0);
        at = cpu.read32(w + SLAVE_PLACE);
    }
}

// ---- STG00.BIN's copies of SGL's rotation scroll set-up ------------------------------------------

/// slSpriteType (STG00.BIN's copy): the sprite type (SPCTL's SPTYPE, bits 0-3).
pub fn sl_sprite_type(cpu: &mut Cpu, t: u32) {
    let w = cpu.gbr();
    let v = (cpu.read16(w + SPCTL) as i16 as i32 as u32 & !0xf) | t;
    cpu.write16(w + SPCTL, v as u16);
    cpu.leave(&[(0, v), (1, !0xf)]);
}

/// slPlaneRA, slOverRA, slPlaneRB, slOverRB (STG00.BIN's copies): rotation parameter A's and
/// B's plane size and over mode, two bits each in PLSZ's high byte.
const PLSZ_HIGH: u32 = 0xfa;

fn plsz_high(cpu: &mut Cpu, keep: u32, v: u32) {
    let w = cpu.gbr();
    let p = (cpu.read8(w + PLSZ_HIGH) as i8 as i32 as u32 & keep) | v;
    cpu.write8(w + PLSZ_HIGH, p as u8);
    cpu.leave(&[(0, p)]);
}

pub fn sl_plane_ra(cpu: &mut Cpu, size: u32) {
    plsz_high(cpu, 0xfc, size);
}

pub fn sl_over_ra(cpu: &mut Cpu, mode: u32) {
    plsz_high(cpu, 0xf3, mode << 2);
    cpu.leave(&[(4, mode << 2)]);
}

pub fn sl_plane_rb(cpu: &mut Cpu, size: u32) {
    plsz_high(cpu, 0xcf, size << 4);
    cpu.leave(&[(4, size << 4)]);
}

pub fn sl_over_rb(cpu: &mut Cpu, mode: u32) {
    plsz_high(cpu, 0x3f, (mode << 8) >> 2);
    cpu.leave(&[(4, (mode << 8) >> 2)]);
}

/// slRparaMode (STG00.BIN's copy): RPMD.
const RPMD: u32 = 0x170;

pub fn sl_rpara_mode(cpu: &mut Cpu, mode: u32) {
    let w = cpu.gbr();
    cpu.write16(w + RPMD, mode as u16);
    cpu.leave(&[(0, mode)]);
}

/// The rotation parameters' tables (A's, B's 0x68 bytes on) as SGL keeps them, and the current
/// one (slCurRpara: STG00.BIN's copy).
const RPARA_A: u32 = 0x060f_fe1c;
const RPARA_B: u32 = 0x060f_fe84;
const CURRENT_RPARA: u32 = 0x1e0;

pub fn sl_cur_rpara(cpu: &mut Cpu, b: u32) {
    let w = cpu.gbr();
    let at = if b != 0 { RPARA_A + 0x68 } else { RPARA_A };
    cpu.write32(w + CURRENT_RPARA, at);
    cpu.leave(&[(0, at)]);
    cpu.leave_t(b == 0);
}

/// slRparaInitSet (STG00.BIN's copy): the rotation parameters' table at `tables` (its address
/// kept, and in RPTA in words), both parameters set to their start (rpara_init), A current.
const ROTATION_TABLES: u32 = 0x214;
const RPTA: u32 = 0x17c;
const RPARA_HALF_FROM: u32 = 0x80;
const RPARA_WORD_FROM: u32 = 0x68;

pub fn sl_rpara_init_set(cpu: &mut Cpu, tables: u32) {
    rpara_init_set(cpu, tables, |cpu| nested!(cpu, 0x0608_0ec8, rpara_init(cpu)));
}

/// slRparaInitSet (STG01.BIN's copy, its own rpara_init's).
pub fn sl_rpara_init_set_stg01(cpu: &mut Cpu, tables: u32) {
    rpara_init_set(cpu, tables, |cpu| nested!(cpu, 0x0609_f260, rpara_init(cpu)));
}

/// slRparaInitSet (STG04.BIN's copy, its own rpara_init's).
pub fn sl_rpara_init_set_stg04(cpu: &mut Cpu, tables: u32) {
    rpara_init_set(cpu, tables, |cpu| nested!(cpu, 0x0608_7b30, rpara_init(cpu)));
}

/// slRparaInitSet (STG02.BIN's copy, its own rpara_init's).
pub fn sl_rpara_init_set_stg02(cpu: &mut Cpu, tables: u32) {
    rpara_init_set(cpu, tables, |cpu| nested!(cpu, 0x060a_2558, rpara_init(cpu)));
}

fn rpara_init_set(cpu: &mut Cpu, tables: u32, init: impl Fn(&mut Cpu)) {
    let w = cpu.gbr();
    cpu.write32(w + ROTATION_TABLES, tables);
    cpu.write32(w + RPTA, tables >> 1);
    let half = cpu.read32(w + RPARA_HALF_FROM) >> 1;
    let word = cpu.read32(w + RPARA_WORD_FROM);
    cpu.write32(w + CURRENT_RPARA, RPARA_A);
    cpu.leave(&[(2, half), (3, word)]);
    for table in [RPARA_A, RPARA_B] {
        cpu.leave(&[(0, table)]);
        init(cpu);
    }
    // its GBR, pushed at the start, back
    cpu.sh2.gbr = w;
}

/// A rotation parameter's table at r0 (SGL's convention here: r0, with r2 and r3 the caller's)
/// to its start: places and the coefficient's start 0, the matrix's diagonal and steps 1 (16.16),
/// the screen's centre from r2 (twice), r3's halves swapped (twice), a word at +100 cleared. GBR
/// is left at the table, as the SH-2 code leaves it.
pub fn rpara_init(cpu: &mut Cpu) {
    let (table, half, word) = (cpu.reg(0), cpu.reg(2), cpu.reg(3));
    cpu.sh2.gbr = table;
    for at in [0, 4, 8, 12, 24, 32, 36, 40, 48, 68, 72] {
        cpu.write32(table + at, 0);
    }
    for at in [16, 20, 28, 44, 76, 80] {
        cpu.write32(table + at, 0x1_0000);
    }
    for at in [52, 60] {
        cpu.write32(table + at, half);
    }
    let swapped = word.rotate_left(16);
    for at in [56, 64] {
        cpu.write16(table + at, swapped as u16);
    }
    cpu.write16(table + 100, 0);
    cpu.leave(&[(0, 0)]);
}

/// slMakeKtable (STG00.BIN's copy): a coefficient table at `table` (its address kept): 0x4002
/// longs of "none" (the top bit), then 0x3ffe of 2^32 over a divisor stepping by 0x200 from
/// 0x400 (each on the divide unit), the low 23 bits, with a top byte that steps down from 0x7d by
/// 2 while it can.
const KTABLE_AT: u32 = 0x218;
const NO_COEFFICIENT: u32 = 0xff00_0000;
const KTABLE_NONE: u32 = 0x4002;
const KTABLE_STEPS: u32 = 0x3ffe;

pub fn sl_make_ktable(cpu: &mut Cpu, table: u32) {
    let w = cpu.gbr();
    cpu.write32(w + KTABLE_AT, table);
    let mut at = table;
    for _ in 0..KTABLE_NONE {
        cpu.write32(at, NO_COEFFICIENT);
        at += 4;
    }
    let (mut divisor, mut top, mut v, mut r1) = (0x200u32, 0x7f00_0000u32, 0, 0);
    for _ in 0..KTABLE_STEPS {
        divisor += 0x200;
        v = divide64(cpu, divisor, 1 << 32) & 0x7f_ffff;
        r1 = 0;
        if top as i32 >= 0x20_0000 {
            top -= 0x20_0000;
            r1 = top & 0x7f00_0000;
            v |= r1;
        }
        cpu.write32(at, v);
        at += 4;
    }
    cpu.leave(&[(0, v), (1, r1), (2, 1), (3, 0xffff_ff00), (4, at), (5, 0), (6, 0x200), (7, divisor)]);
    cpu.leave_t(true);
}

/// slKtableRA, slKtableRB (STG00.BIN's copies): the parameter's coefficient table at `table` in
/// `mode` (KTCTL's byte, kept in the parameter's table too): KTAOF's bits from the table's
/// address (in words or longs by mode bit 1), the table's offset kept; for A, the banks VRAM gave
/// the coefficients taken back; with mode bit 5, the table's bank given to them (rotation_bank).
const KTCTL: u32 = 0x174;
const KTAOF: u32 = 0x176;
const KTABLE_A_KEPT: u32 = 0x27c;
const KTABLE_A_OFFSET: u32 = 0x270;
const KTABLE_B_OFFSET: u32 = 0x2d8;
const RPARA_A_MODE: u32 = 0x060f_fe80;
const RPARA_B_TABLE: u32 = 0x060f_fee4;
const RPARA_B_MODE: u32 = 0x060f_fee8;
const BANK_COEFFICIENTS: u32 = 1;

fn ktable_shifted(table: u32, mode: u32) -> u32 {
    if mode & 2 != 0 { table >> 1 } else { table >> 2 }
}

pub fn sl_ktable_ra(cpu: &mut Cpu, table: u32, mode: u32) {
    let w = cpu.gbr();
    let c = (cpu.read16(w + KTCTL) as i16 as i32 as u32 >> 8 << 8) | mode;
    cpu.write16(w + KTCTL, c as u16);
    cpu.write16(RPARA_A_MODE, mode as u16);
    cpu.write32(w + KTABLE_A_KEPT, table);
    let page = (ktable_shifted(table, mode) >> 16) & 7;
    let a = (cpu.read16(w + KTAOF) as i16 as i32 as u32 >> 8 << 8) | page;
    cpu.write16(w + KTAOF, a as u16);
    cpu.write32(w + KTABLE_A_OFFSET, ktable_shifted(table, mode) << 16);
    // RAMCTL's banks given to coefficients (1) taken back
    let mut r = cpu.read16(w + RAMCTL) as i16 as i32 as u32;
    let mut r6 = 0;
    for bank in 0..4 {
        let mask = 3 << (2 * bank);
        r6 = r & mask;
        if r6 == BANK_COEFFICIENTS << (2 * bank) {
            r6 = !mask;
            r &= r6;
        }
    }
    cpu.write16(w + RAMCTL, r as u16);
    cpu.leave(&[(6, r6)]);
    if mode & 0x20 != 0 {
        cpu.call(ROTATION_BANK, &[table, BANK_COEFFICIENTS]);
    } else {
        cpu.leave(&[(0, 0x20), (1, 0), (2, 0x300), (3, 0x100)]);
        cpu.leave_t(true);
    }
}

pub fn sl_ktable_rb(cpu: &mut Cpu, table: u32, mode: u32) {
    let w = cpu.gbr();
    let c = (cpu.read16(w + KTCTL) as i16 as i32 as u32 & 0xff) | mode << 8;
    cpu.write16(w + KTCTL, c as u16);
    cpu.write16(RPARA_B_MODE, mode as u16);
    cpu.write32(RPARA_B_TABLE, table);
    let page = ((ktable_shifted(table, mode) >> 16) & 7) << 8;
    let a = (cpu.read16(w + KTAOF) as i16 as i32 as u32 & 0xff) | page;
    cpu.write16(w + KTAOF, a as u16);
    let offset = ktable_shifted(table, mode) << 16;
    cpu.write32(w + KTABLE_B_OFFSET, offset);
    if mode & 0x20 != 0 {
        cpu.call(ROTATION_BANK, &[table, BANK_COEFFICIENTS]);
    } else {
        cpu.leave(&[(0, 0x20), (1, 2), (2, offset), (3, 7)]);
        cpu.leave_t(true);
    }
}

/// slMapRB (STG00.BIN's copy): rotation parameter B's sixteen planes all on the map at `a`, its
/// VRAM bank given to the pattern names (slMapRbg0's for A).
const RB_MAP: u32 = 0x210;
const MPABRB: u32 = 0x120;

pub fn sl_map_rb(cpu: &mut Cpu, a: u32) {
    let w = cpu.gbr();
    cpu.write32(w + RB_MAP, a);
    cpu.call(ROTATION_BANK, &[a, BANK_NAMES]);
    let s = page_shift(cpu, PNCR, CHCTLB, 8);
    let offset = ((a >> (s + 6)) & 7) << 4;
    let m = cpu.read8(w + MPOFR_LOW) as i8 as i32 as u32;
    cpu.write8(w + MPOFR_LOW, ((m & 0x0f) | offset) as u8);
    let page = (a >> s) & 63;
    let v = page << 8 | page;
    for k in 0..8 {
        cpu.write16(w + MPABRB + 2 * k, v as u16);
    }
    cpu.leave(&[(0, v), (1, 63), (2, offset), (3, s), (4, page)]);
    cpu.leave_t(s & 1 == 0);
}

/// slMapRA and slMapRB from a table (STG01.BIN's copies): the rotation parameter's 16 planes' maps
/// from `map` (a byte each, the map's number in 2 KB pages): the first set as the parameter's map
/// (RA through slMapRbg0, RB through the copy's slMapRB), then each pair of planes' pages packed
/// in a word of MPABRA's or MPABRB's image (halved with 2-word pattern names, quartered with 1x1
/// cells: RBG0's pattern name control's top bit and character size).
pub fn sl_map_ra_table(cpu: &mut Cpu, map: u32) {
    let first = (cpu.read8(map) as u32) << 11;
    nested!(cpu, 0x0607_1c4c, sl_map_rbg0(cpu, first));
    map_table(cpu, map, VDP2_IMAGE - IMAGE + MPABRA);
}

pub fn sl_map_rb_table(cpu: &mut Cpu, map: u32) {
    let first = ((cpu.read8(map) as i8 as i32) << 11) as u32;
    nested!(cpu, 0x0609_f54c, sl_map_rb(cpu, first));
    map_table(cpu, map, VDP2_IMAGE - IMAGE + MPABRB);
}

/// slMapRB from a table (STG02.BIN's copy, its own slMapRB's).
pub fn sl_map_rb_table_stg02(cpu: &mut Cpu, map: u32) {
    let first = ((cpu.read8(map) as i8 as i32) << 11) as u32;
    nested!(cpu, 0x060a_28a4, sl_map_rb(cpu, first));
    map_table(cpu, map, VDP2_IMAGE - IMAGE + MPABRB);
}

fn map_table(cpu: &mut Cpu, map: u32, at: u32) {
    let w = cpu.gbr();
    let one_word = cpu.read16(w + IMAGE + PNCR) & 0x8000 != 0;
    let size = ((cpu.read8(w + CHCTLB) as i8 as i32 as u32) << 1) | one_word as u32;
    let (mut a, mut b) = (0, 0);
    for k in 0..8 {
        a = cpu.read8(map + 2 * k) as u32;
        b = cpu.read8(map + 2 * k + 1) as u32;
        if size & 2 == 0 {
            a >>= 2;
            b >>= 2;
        }
        if size & 1 == 0 {
            a >>= 1;
            b >>= 1;
        }
        cpu.write16(at + 2 * k, (b << 8 | a) as u16);
    }
    cpu.leave(&[(0, size), (1, 0), (2, b << 8 | a), (3, at + 16), (4, b << 8), (6, map + 16)]);
    cpu.leave_t(true);
}

/// The colour offsets enabled on the screens of `screens`, offset B on them (CLOFEN and CLOFSL
/// both ORed with it; STG01.BIN's copy).
pub fn sl_col_offset_b_on(cpu: &mut Cpu, screens: u32) {
    let w = cpu.gbr();
    let s = screens & 0xffff;
    let v = cpu.read32(w + CLOFEN) | s | s << 16;
    cpu.write32(w + CLOFEN, v);
    cpu.leave(&[(0, v), (4, s << 16)]);
}

/// slScrWindow0 (STG01.BIN's copy): window 0's corners (x doubled unless the TV mode is
/// high-resolution: TVMD's bit 1) in WPSX0-WPEY0, its line window table none (LWTA0).
const WPSX0: u32 = 0x180;
const WPEX0: u32 = 0x184;
const LWTA0: u32 = 0x198;

pub fn sl_scr_window0(cpu: &mut Cpu, left: u32, top: u32, right: u32, bottom: u32) {
    let w = cpu.gbr();
    let hi = cpu.read16(w + TVMD) & 2 != 0;
    // T: TVMD's test, or the last doubling's carry
    let t = !hi && right & 0x8000_0000 != 0;
    let (left, right) = if hi { (left, right) } else { (left << 1, right << 1) };
    cpu.write32(w + WPSX0, left << 16 | top & 0xffff);
    cpu.write32(w + WPEX0, right << 16 | bottom & 0xffff);
    cpu.write32(w + LWTA0, 0);
    cpu.leave(&[(0, 0), (4, left), (6, right)]);
    cpu.leave_t(t);
}

/// slLineWindow0 (STG01.BIN's copy): window 0's line window table at `table` (LWTA0: in words,
/// its top bit kept as the enable).
pub fn sl_line_window0(cpu: &mut Cpu, table: u32) {
    let w = cpu.gbr();
    let v = (table as i32 >> 1) as u32;
    cpu.write32(w + LWTA0, v);
    cpu.leave(&[(0, v), (4, table << 1)]);
    cpu.leave_t(table & 1 != 0);
}

/// slLineScrollTable0 (STG02.BIN's copy): NBG0's line scroll table at `table` (LSTA0, in words).
const LSTA0: u32 = 0x160;

pub fn sl_line_table_nbg0(cpu: &mut Cpu, table: u32) {
    let w = cpu.gbr();
    cpu.write32(w + LSTA0, table >> 1);
    cpu.leave(&[(0, table >> 1)]);
    cpu.leave_t(table & 1 != 0);
}

/// The current rotation parameter's (slCurRpara's) screen centre moved so that the screen
/// point (`x`, `y`) lies at its start (STG02.BIN's copy): Mx and My are the point less the
/// table's Px and Py (words at +0x3c and +0x3e, put in the high half). GBR is the table's while
/// it works.
const RP_PX_WORD: u32 = 0x3c;
const RP_PY_WORD: u32 = 0x3e;
const RP_MY: u32 = 0x48;

pub fn sl_rpara_place(cpu: &mut Cpu, x: u32, y: u32) {
    let w = cpu.gbr();
    let table = cpu.read32(w + ROTATION_PARAMS);
    let px = (cpu.read16(table + RP_PX_WORD) as i16 as i32 as u32) << 16;
    let x = x.wrapping_sub(px);
    cpu.write32(table + RP_MX, x);
    let py = (cpu.read16(table + RP_PY_WORD) as i16 as i32 as u32) << 16;
    let y = y.wrapping_sub(py);
    cpu.write32(table + RP_MY, y);
    cpu.leave(&[(0, y), (4, x), (5, y)]);
}

/// The current rotation parameter's matrix turned by `a` (STG02.BIN's copy): its busy byte
/// cleared while the sine and cosine are made (sl_sin_cos), then C and D the sine and minus the
/// sine, B and E the cosine, and the busy byte set.
const RP_TURN_SIN: u32 = 40;
const RP_TURN_MINUS_SIN: u32 = 32;
const RP_TURN_COS: u32 = 28;
const RP_TURN_COS_AGAIN: u32 = 44;

pub fn sl_rpara_turn(cpu: &mut Cpu, a: u32) {
    let w = cpu.gbr();
    cpu.write8(w + ROTATION_BUSY, 0);
    let table = cpu.read32(w + ROTATION_PARAMS);
    nested!(cpu, SL_SIN_COS, sl_sin_cos(cpu, a));
    let (s, c) = (cpu.reg(4), cpu.reg(5));
    cpu.write32(table + RP_TURN_SIN, s);
    let minus = s.wrapping_neg();
    cpu.write32(table + RP_TURN_MINUS_SIN, minus);
    cpu.write32(table + RP_TURN_COS, c);
    cpu.write8(w + ROTATION_BUSY, 1);
    cpu.write32(table + RP_TURN_COS_AGAIN, c);
    cpu.leave(&[(0, 1), (4, minus), (7, table)]);
}

/// The current rotation parameter's scales Kx and Ky (+0x4c, +0x50) set to `x` and `y` (STG02.BIN's
/// copy).
pub fn sl_rpara_scale(cpu: &mut Cpu, x: u32, y: u32) {
    let w = cpu.gbr();
    let table = cpu.read32(w + ROTATION_PARAMS);
    cpu.write32(table + 76, x);
    cpu.write32(table + 80, y);
    cpu.leave(&[(0, table), (1, 80)]);
}
