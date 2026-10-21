//! The game's arithmetic helpers (assembly beside its main program): 16.16 fixed-point multiply and
//! divide, and division on the SH-2's divide unit, which the game's division goes through.

use super::nested;
use super::Cpu;

/// The divide unit: divisor, 32-bit dividend (writing it divides), and the 64-bit dividend's
/// halves (writing the low half divides), which hold the remainder and the quotient after.
const DVSR: u32 = 0xFFFF_FF00;
const DVDNT: u32 = 0xFFFF_FF04;
const DVDNTH: u32 = 0xFFFF_FF10;
const DVDNTL: u32 = 0xFFFF_FF14;

/// `a * b` in 16.16 fixed point (the 64-bit product's middle).
pub fn fixed_mul(_cpu: &mut Cpu, a: u32, b: u32) -> u32 {
    ((a as i32 as i64 * b as i32 as i64) >> 16) as u32
}

/// `a % b` (signed), on the divide unit.
pub fn remainder(cpu: &mut Cpu, a: u32, b: u32) -> u32 {
    cpu.write32(DVSR, b);
    cpu.write32(DVDNT, a);
    cpu.read32(DVDNTH)
}

/// `a / b` (signed), on the divide unit.
pub fn quotient(cpu: &mut Cpu, a: u32, b: u32) -> u32 {
    cpu.write32(DVSR, b);
    cpu.write32(DVDNT, a);
    cpu.read32(DVDNTL)
}

/// `a / b` in 16.16 fixed point, on the divide unit (`a << 16` divided as 64 bits).
pub fn fixed_div(cpu: &mut Cpu, a: u32, b: u32) -> u32 {
    cpu.write32(DVSR, b);
    cpu.write32(DVDNTH, (a >> 16) as i16 as i32 as u32);
    cpu.write32(DVDNTL, a << 16);
    cpu.read32(DVDNTL)
}

/// The random numbers: the seed, and a copy of it made as it is set (not known why).
const RANDOM: u32 = 0x0607_3c74;
const RANDOM_COPY: u32 = 0x060f_ff3c;

/// The random numbers from `seed` (a demo sets 0, so it plays the same each time).
pub fn random_seed(cpu: &mut Cpu, seed: u32) {
    cpu.write32(RANDOM, seed);
    cpu.write32(RANDOM_COPY, seed);
}

/// The next random number: the seed times 5 plus 0x3573; below `n` (its remainder) unless `n` is 0.
pub fn random(cpu: &mut Cpu, n: u32) -> u32 {
    let r = cpu.read32(RANDOM).wrapping_mul(5).wrapping_add(0x3573);
    cpu.write32(RANDOM, r);
    if n == 0 {
        return r;
    }
    nested!(cpu, 0x0600_6a6e, remainder(cpu, r, n))
}

/// The difficulty (the first setting, backup.rs: 0-4, the demos' 2 the middle).
const DIFFICULTY: u32 = 0x0600_ed84;
const MIDDLE: i32 = 2;

/// `x` scaled by the difficulty: (difficulty - 2) * x / 2; with `x` 0, the difficulty itself.
pub fn by_difficulty(cpu: &mut Cpu, x: u32) -> u32 {
    let d = cpu.read8(DIFFICULTY) as i8 as i32;
    if x == 0 {
        return d as u32;
    }
    ((d - MIDDLE).wrapping_mul(x as i32) / 2) as u32
}

/// `x` held within -`limit`..=`limit`.
pub fn clamp(_cpu: &mut Cpu, x: u32, limit: u32) -> u32 {
    let (x, limit) = (x as i32, limit as i32);
    if x > limit {
        limit as u32
    } else if x < limit.wrapping_neg() {
        limit.wrapping_neg() as u32
    } else {
        x as u32
    }
}

/// `x` pushed out to at least `least` from zero (on its side; 0 counts as positive).
pub fn at_least(_cpu: &mut Cpu, x: u32, least: u32) -> u32 {
    let (x, least) = (x as i32, least as i32);
    let (size, signed) = if x >= 0 { (x, least) } else { (x.wrapping_neg(), least.wrapping_neg()) };
    if least > size { signed as u32 } else { x as u32 }
}


/// SGL's slMulFX and slSin.
const SL_MUL_FX: u32 = 0x0606_e26c;
const SL_SIN: u32 = 0x0606_9a0c;
const ONE: u32 = 0x10000;

/// `a` to `b` by `t` (16.16, 0 to 1), in a line: a(1 - t) + bt.
pub fn lerp(cpu: &mut Cpu, a: u32, b: u32, t: u32) -> u32 {
    let from = cpu.call(SL_MUL_FX, &[a, ONE.wrapping_sub(t)]);
    from.wrapping_add(cpu.call(SL_MUL_FX, &[b, t]))
}

/// lerp eased in and out: by (1 - cos(half a turn by `t`)) / 2.
pub fn ease(cpu: &mut Cpu, a: u32, b: u32, t: u32) -> u32 {
    let angle = ((t as i32 >> 1) - 0x4000) as i16 as i32 as u32;
    let s = (cpu.call(SL_SIN, &[angle]).wrapping_add(ONE) as i32 >> 1) as u32;
    let from = cpu.call(SL_MUL_FX, &[a, ONE.wrapping_sub(s)]);
    from.wrapping_add(cpu.call(SL_MUL_FX, &[b, s]))
}

/// The angles (longs, 16 bits of turn) at `a` and `b` brought onto one side of the turn: when
/// they are more than half a turn apart, each negative one a turn on.
pub fn unwrap_angles(cpu: &mut Cpu, a: u32, b: u32) {
    let x = cpu.read32(a) as i32;
    if x.wrapping_sub(cpu.read32(b) as i32).wrapping_abs() > 0x8000 {
        if x < 0 {
            cpu.write32(a, x.wrapping_add(0x1_0000) as u32);
        }
        let y = cpu.read32(b) as i32;
        if y < 0 {
            cpu.write32(b, y.wrapping_add(0x1_0000) as u32);
        }
    }
}

/// SGL's sine and cosine tables: for each eighth of the turn (by the angle's top three bits) an
/// entry of 8 bytes: the quarter-wave table it reads (words, 16.16 fractions: 0 to 1 over a
/// quarter turn by 2-word steps), what the result is xored with and then added (bytes,
/// sign-extended: -1 and 1 negate it), and what the angle is xored with first (a byte,
/// sign-extended: -1 reads the quarter backward). The cosine's entries are the sine's two on.
const SIN_ENTRIES: u32 = 0x0606_9a98;
const COS_ENTRIES: u32 = 0x0606_9aa8;
const QUARTER_STEPS: u32 = 0x3ffe;

fn trig(cpu: &mut Cpu, entries: u32, a: u32) -> u32 {
    let e = entries + ((a >> 13) & 7) * 8;
    let flip = cpu.read8(e + 6) as i8 as i32 as u32;
    let table = cpu.read32(e);
    let v = cpu.read16(table + ((a ^ flip) & QUARTER_STEPS)) as u32;
    let x = cpu.read8(e + 4) as i8 as i32 as u32;
    let add = cpu.read8(e + 5) as i8 as i32 as u32;
    (v ^ x).wrapping_add(add)
}

/// SGL's slSin: the sine of `a` (16 bits of turn), 16.16.
pub fn sl_sin(cpu: &mut Cpu, a: u32) -> u32 {
    trig(cpu, SIN_ENTRIES, a)
}

/// SGL's slCos.
pub fn sl_cos(cpu: &mut Cpu, a: u32) -> u32 {
    trig(cpu, COS_ENTRIES, a)
}

/// `a` times `b`, 16.16 (the 64-bit product's middle 32 bits: dmuls.l then xtrct).
fn mul_fx(a: u32, b: u32) -> u32 {
    ((a as i32 as i64 * b as i32 as i64) >> 16) as u32
}

/// SGL's slMulFX.
pub fn sl_mul_fx(_cpu: &mut Cpu, a: u32, b: u32) -> u32 {
    mul_fx(a, b)
}

/// The point (x, y longs at `t`) flipped (the flags at +0x14: TP_FLIP_X, TP_FLIP_Y), scaled (by
/// +0xc, +0x10) and turned by the angle at +0x18 (a word; 0 none). (The SH-2 code pushes r11,
/// r12 and PR only to turn it, below its prologue's pushes.)
const TP_SCALES: u32 = 0xc;
const TP_FLAGS: u32 = 0x14;
const TP_ANGLE: u32 = 0x18;
const TP_FLIP_X: u32 = 0x10;
const TP_FLIP_Y: u32 = 0x20;

pub fn turn_point(cpu: &mut Cpu, t: u32) {
    let flags = cpu.read32(t + TP_FLAGS);
    let mut p = [0; 2];
    for (k, flip) in [TP_FLIP_X, TP_FLIP_Y].into_iter().enumerate() {
        let v = cpu.read32(t + 4 * k as u32);
        let v = if flags & flip != 0 { v.wrapping_neg() } else { v };
        let scale = cpu.read32(t + TP_SCALES + 4 * k as u32);
        p[k] = mul_fx(v, scale);
    }
    let a = cpu.read16(t + TP_ANGLE);
    let [mut x, mut y] = p;
    if a != 0 {
        let (r11, r12, pr) = (cpu.reg(11), cpu.reg(12), cpu.pr());
        cpu.push(&[pr, r12, r11]);
        let a = a as i16 as i32 as u32;
        let c = nested!(cpu, 0x0606_9a30, sl_cos(cpu, a));
        let s = nested!(cpu, 0x0606_9a0c, sl_sin(cpu, a));
        x = mul_fx(p[0], c).wrapping_sub(mul_fx(p[1], s));
        y = mul_fx(p[1], c).wrapping_add(mul_fx(p[0], s));
        cpu.pop(12);
    }
    cpu.write32(t, x);
    cpu.write32(t + 4, y);
}
