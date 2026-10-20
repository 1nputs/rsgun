//! Instruction tests from the manual's definitions. The oracle's traces check the core on real code
//! (stv-check trace); these cover what is easy to get wrong and what play may not reach.

use super::*;

/// 64 KB of big-endian RAM at address 0, the code at 0x1000.
struct Ram(Vec<u8>);

impl Ram {
    fn with_code(code: &[u16]) -> Ram {
        let mut ram = Ram(vec![0; 0x10000]);
        for (i, w) in code.iter().enumerate() {
            ram.write16(0x1000 + 2 * i as u32, *w);
        }
        ram
    }
}

impl Bus for Ram {
    fn read8(&mut self, a: u32) -> u8 {
        self.0[a as usize & 0xFFFF]
    }
    fn read16(&mut self, a: u32) -> u16 {
        u16::from_be_bytes([self.read8(a), self.read8(a + 1)])
    }
    fn read32(&mut self, a: u32) -> u32 {
        ((self.read16(a) as u32) << 16) | self.read16(a + 2) as u32
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.0[a as usize & 0xFFFF] = v;
    }
    fn write16(&mut self, a: u32, v: u16) {
        self.write8(a, (v >> 8) as u8);
        self.write8(a + 1, v as u8);
    }
    fn write32(&mut self, a: u32, v: u32) {
        self.write16(a, (v >> 16) as u16);
        self.write16(a + 2, v as u16);
    }
    fn fetch16(&mut self, a: u32) -> u16 {
        self.read16(a)
    }
}

fn cpu() -> Sh2 {
    Sh2 { pc: 0x1000, r: { let mut r = [0; 16]; r[15] = 0x8000; r }, ..Sh2::default() }
}

fn run(cpu: &mut Sh2, ram: &mut Ram, steps: usize) {
    for _ in 0..steps {
        cpu.step(ram);
    }
}

/// A small xorshift, so the tests need no crate.
fn rng(seed: &mut u64) -> u32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 7;
    *seed ^= *seed << 17;
    (*seed >> 16) as u32
}

#[test]
fn unsigned_division_by_div1() {
    // div0u; 32 × (rotcl r1; div1 r3,r2); rotcl r1: r1 = r1 / r3 with r2 = 0 (the manual's 64/32)
    let mut code = vec![0x0019];
    for _ in 0..32 {
        code.extend([0x4124, 0x3234]);
    }
    code.push(0x4124);
    let mut seed = 0x1234_5678_9ABC_DEF1;
    for _ in 0..500 {
        let a = rng(&mut seed);
        let b = rng(&mut seed) >> (rng(&mut seed) % 31);
        if b == 0 {
            continue;
        }
        let mut c = cpu();
        c.r[1] = a;
        c.r[2] = 0;
        c.r[3] = b;
        let mut ram = Ram::with_code(&code);
        run(&mut c, &mut ram, code.len());
        assert_eq!(c.r[1], a / b, "{a:#x} / {b:#x}");
    }
}

#[test]
fn signed_division_by_div1() {
    // The manual's signed 32/32 division: r1 / r0, quotient in r1.
    // mov r1,r2 (0x6213); rotcl r2 (0x4224); subc r2,r2 (0x322A): r2 = the dividend's sign extension
    // div0s r0,r2 (0x2207); 32 × (rotcl r1; div1 r0,r2); rotcl r1; addc r2? (sign fix below)
    let mut code = vec![0x6213, 0x4224, 0x322A, 0x0009, 0x2207];
    for _ in 0..32 {
        code.extend([0x4124, 0x3204]);
    }
    // rotcl r1; mov #0,r3 (0xE300); ... the manual's correction: if the dividend was negative, add 1
    code.extend([0x4124]);
    let mut seed = 0xDEAD_BEEF_0123_4567;
    for _ in 0..500 {
        let a = rng(&mut seed) as i32;
        let b = (rng(&mut seed) as i32) >> (rng(&mut seed) % 30);
        if b == 0 || (a == i32::MIN && b == -1) {
            continue;
        }
        let mut c = cpu();
        c.r[1] = a as u32;
        c.r[0] = b as u32;
        // r2 starts as 0: subc r2,r2 with T from rotcl gives 0 or -1
        let mut ram = Ram::with_code(&code);
        run(&mut c, &mut ram, code.len());
        // Non-restoring signed division gives the quotient in one's complement when it is negative:
        // the manual adds 1 (addc #0 after rotcl) for that case.
        let q = c.r[1] as i32;
        let expect = a / b;
        assert!(q == expect || q == expect - 1 || q.wrapping_add(1) == expect,
            "{a} / {b}: got {q}, expected about {expect}");
    }
}

#[test]
fn addc_subc_chain_64_bits() {
    // clrt; addc r3,r1; addc r4,r2: r2:r1 += r4:r3
    let code = [0x0008, 0x313E, 0x324E];
    let mut seed = 99;
    for _ in 0..200 {
        let (a, b) = ((rng(&mut seed) as u64) << 32 | rng(&mut seed) as u64,
                      (rng(&mut seed) as u64) << 32 | rng(&mut seed) as u64);
        let mut c = cpu();
        c.r[1] = a as u32;
        c.r[2] = (a >> 32) as u32;
        c.r[3] = b as u32;
        c.r[4] = (b >> 32) as u32;
        let mut ram = Ram::with_code(&code);
        run(&mut c, &mut ram, 3);
        let sum = a.wrapping_add(b);
        assert_eq!((c.r[2] as u64) << 32 | c.r[1] as u64, sum);
        assert_eq!(c.t(), a.checked_add(b).is_none());
    }
    // clrt; subc r3,r1; subc r4,r2: r2:r1 -= r4:r3
    let code = [0x0008, 0x313A, 0x324A];
    for _ in 0..200 {
        let (a, b) = ((rng(&mut seed) as u64) << 32 | rng(&mut seed) as u64,
                      (rng(&mut seed) as u64) << 32 | rng(&mut seed) as u64);
        let mut c = cpu();
        c.r[1] = a as u32;
        c.r[2] = (a >> 32) as u32;
        c.r[3] = b as u32;
        c.r[4] = (b >> 32) as u32;
        let mut ram = Ram::with_code(&code);
        run(&mut c, &mut ram, 3);
        assert_eq!((c.r[2] as u64) << 32 | c.r[1] as u64, a.wrapping_sub(b));
        assert_eq!(c.t(), a < b);
    }
}

#[test]
fn negc_negates_64_bits() {
    // clrt; negc r1,r1; negc r2,r2
    let code = [0x0008, 0x611A, 0x622A];
    for a in [0u64, 1, 0xFFFF_FFFF, 0x1_0000_0000, 0x8000_0000_0000_0000, 0x1234_5678_9ABC_DEF0] {
        let mut c = cpu();
        c.r[1] = a as u32;
        c.r[2] = (a >> 32) as u32;
        let mut ram = Ram::with_code(&code);
        run(&mut c, &mut ram, 3);
        assert_eq!((c.r[2] as u64) << 32 | c.r[1] as u64, 0u64.wrapping_sub(a), "{a:#x}");
    }
}

#[test]
fn delayed_branches_run_their_slot() {
    // 1000: bsr +4 (to 100A)   1002: add #1,r0 (slot)   1004: add #16,r0   1006: bra 1006
    // 100A: rts                 100C: add #2,r0 (slot)
    let code = [0xB003, 0x7001, 0x7010, 0xAFFE, 0x0009, 0x000B, 0x7002];
    let mut c = cpu();
    let mut ram = Ram::with_code(&code);
    run(&mut c, &mut ram, 2);
    assert_eq!((c.pc, c.pr, c.r[0]), (0x100A, 0x1004, 1));
    run(&mut c, &mut ram, 3);
    assert_eq!((c.pc, c.r[0]), (0x1006, 0x13));
}

#[test]
fn conditional_branches() {
    // 1000: sett; bt +2 (to 1008); add #1,r0; add #2,r0; 1008: clrt; bf/s +2 (to 1010) with slot add #4
    let code = [0x0018, 0x8901, 0x7001, 0x7002, 0x0008, 0x8F01, 0x7004, 0x7008, 0x7010];
    let mut c = cpu();
    let mut ram = Ram::with_code(&code);
    run(&mut c, &mut ram, 2);
    assert_eq!(c.pc, 0x1008);
    run(&mut c, &mut ram, 3); // clrt; bf/s; its slot
    assert_eq!((c.pc, c.r[0]), (0x1010, 4));
}

#[test]
fn pc_relative_loads() {
    // 1000: mov.l @(4,PC),r1 -> (1004 & ~3) + 4 = 1008   1002: mov.w @(2,PC),r2 -> 1006 + 2 = 1008
    // 1004: mova @(4,PC),r0 -> (1008 & ~3) + 4 = 100C
    let code = [0xD101, 0x9201, 0xC701, 0x0009, 0x8765, 0x4321, 0x0009];
    let mut c = cpu();
    let mut ram = Ram::with_code(&code);
    run(&mut c, &mut ram, 3);
    assert_eq!(c.r[1], 0x8765_4321);
    assert_eq!(c.r[2], 0xFFFF_8765);
    assert_eq!(c.r[0], 0x100C);
}

#[test]
fn mac_l_accumulates_and_saturates() {
    // clrmac; mac.l @r1+,@r2+ twice
    let code = [0x0028, 0x021F, 0x021F];
    let mut c = cpu();
    let mut ram = Ram::with_code(&code);
    for (i, (a, b)) in [(-3i32, 7i32), (0x4000_0000, 0x4000_0000)].iter().enumerate() {
        ram.write32(0x2000 + 4 * i as u32, *a as u32);
        ram.write32(0x3000 + 4 * i as u32, *b as u32);
    }
    c.r[1] = 0x3000; // Rm
    c.r[2] = 0x2000; // Rn
    run(&mut c, &mut ram, 3);
    let mac = ((c.mach as u64) << 32 | c.macl as u64) as i64;
    assert_eq!(mac, -21 + (1i64 << 60));
    assert_eq!((c.r[1], c.r[2]), (0x3008, 0x2008));
    // With S set the 48-bit result saturates.
    let mut c = cpu();
    c.sr = SR_S;
    c.r[1] = 0x3000;
    c.r[2] = 0x2000;
    let mut ram2 = Ram::with_code(&code);
    for (i, (a, b)) in [(-3i32, 7i32), (0x4000_0000, 0x4000_0000)].iter().enumerate() {
        ram2.write32(0x2000 + 4 * i as u32, *a as u32);
        ram2.write32(0x3000 + 4 * i as u32, *b as u32);
    }
    run(&mut c, &mut ram2, 3);
    let mac = ((c.mach as u64) << 32 | c.macl as u64) as i64;
    assert_eq!(mac, 0x7FFF_FFFF_FFFF);
}

#[test]
fn shifts_rotates_and_tests() {
    let mut c = cpu();
    // rotcr r1 with T=1: 0x0000_0002 -> 0x8000_0001, T=0
    let mut ram = Ram::with_code(&[0x0018, 0x4125, 0x4221, 0x4310, 0x2C4C]);
    c.r[1] = 2;
    c.r[2] = 0x8000_0003; // shar: 0xC000_0001, T=1
    c.r[3] = 1; // dt: 0, T=1
    c.r[4] = 0x1122_3344;
    c.r[12] = 0x5566_3377; // cmp/str r4,r12: the third bytes match
    run(&mut c, &mut ram, 2);
    assert_eq!((c.r[1], c.t()), (0x8000_0001, false));
    run(&mut c, &mut ram, 1);
    assert_eq!((c.r[2], c.t()), (0xC000_0001, true));
    run(&mut c, &mut ram, 1);
    assert_eq!((c.r[3], c.t()), (0, true));
    run(&mut c, &mut ram, 1);
    assert!(c.t());
}

#[test]
fn trapa_rte_and_interrupts() {
    // 1000: trapa #0x20   1002: add #1,r0
    // handler at 1100: add #2,r0; rte; nop
    let mut ram = Ram::with_code(&[0xC320, 0x7001]);
    ram.write16(0x1100, 0x7002);
    ram.write16(0x1102, 0x002B);
    ram.write16(0x1104, 0x0009);
    ram.write32(0x0080, 0x1100); // VBR 0 + 0x20 × 4
    let mut c = cpu();
    c.sr = 0x0F0;
    run(&mut c, &mut ram, 1);
    assert_eq!((c.pc, c.r[15]), (0x1100, 0x7FF8));
    assert_eq!((ram.read32(0x7FF8), ram.read32(0x7FFC)), (0x1002, 0x0F0));
    run(&mut c, &mut ram, 3);
    assert_eq!((c.pc, c.r[15], c.r[0], c.sr), (0x1002, 0x8000, 2, 0x0F0));
    // An interrupt takes the vector and sets the mask to its level.
    ram.write32(0x40 * 4, 0x1100);
    c.sr = 0;
    c.interrupt(&mut ram, 0x40, 0xF);
    assert_eq!((c.pc, c.level()), (0x1100, 0xF));
}

#[test]
fn branch_in_a_slot_is_slot_illegal() {
    // 1000: bra 1000 with bra in its slot
    let mut ram = Ram::with_code(&[0xAFFE, 0xAFFE]);
    ram.write32(VEC_SLOT_ILLEGAL * 4, 0x1200);
    let mut c = cpu();
    run(&mut c, &mut ram, 2);
    assert_eq!(c.pc, 0x1200);
    assert_eq!(ram.read32(0x7FF8), 0x1000);
}

#[test]
fn gbr_byte_operations() {
    // tst.b #0x0F,@(r0,gbr); and.b #0xF0,@(r0,gbr); xor.b #0x0F,@(r0,gbr); or.b #0x01,@(r0,gbr)
    let mut ram = Ram::with_code(&[0xCC0F, 0xCDF0, 0xCE0F, 0xCF01]);
    ram.write8(0x2005, 0xA5);
    let mut c = cpu();
    c.gbr = 0x2000;
    c.r[0] = 5;
    run(&mut c, &mut ram, 1);
    assert!(!c.t());
    run(&mut c, &mut ram, 1);
    assert_eq!(ram.read8(0x2005), 0xA0);
    run(&mut c, &mut ram, 2);
    assert_eq!(ram.read8(0x2005), 0xAF);
}

#[test]
fn subv_rotr_shal_dmulu() {
    // subv r2,r1; rotr r3; shal r4; dmulu.l r5,r6
    let mut ram = Ram::with_code(&[0x312B, 0x4305, 0x4420, 0x3655]);
    let mut c = cpu();
    c.r[1] = 0x8000_0000;
    c.r[2] = 1; // overflow: most negative - 1
    c.r[3] = 0x0000_0003;
    c.r[4] = 0x4000_0001;
    c.r[5] = 0xFFFF_FFFF;
    c.r[6] = 0xFFFF_FFFF;
    run(&mut c, &mut ram, 1);
    assert_eq!((c.r[1], c.t()), (0x7FFF_FFFF, true));
    run(&mut c, &mut ram, 1);
    assert_eq!((c.r[3], c.t()), (0x8000_0001, true));
    run(&mut c, &mut ram, 1);
    assert_eq!((c.r[4], c.t()), (0x8000_0002, false));
    run(&mut c, &mut ram, 1);
    assert_eq!((c.mach, c.macl), (0xFFFF_FFFE, 0x0000_0001));
}

#[test]
fn mac_w_accumulates() {
    // clrmac; mac.w @r1+,@r2+ twice: 3 × -4 + 100 × 200
    let mut ram = Ram::with_code(&[0x0028, 0x421F, 0x421F]);
    for (i, (a, b)) in [(3i16, -4i16), (100, 200)].iter().enumerate() {
        ram.write16(0x3000 + 2 * i as u32, *a as u16);
        ram.write16(0x2000 + 2 * i as u32, *b as u16);
    }
    let mut c = cpu();
    c.r[1] = 0x3000;
    c.r[2] = 0x2000;
    run(&mut c, &mut ram, 3);
    assert_eq!(((c.mach as u64) << 32 | c.macl as u64) as i64, -12 + 20000);
    assert_eq!((c.r[1], c.r[2]), (0x3004, 0x2004));
}

#[test]
fn sleep_waits_for_an_interrupt() {
    // sleep; add #1,r0
    let mut ram = Ram::with_code(&[0x001B, 0x7001]);
    ram.write16(0x1100, 0x7010); // handler: add #16,r0
    ram.write32(0x41 * 4, 0x1100);
    let mut c = cpu();
    run(&mut c, &mut ram, 5);
    assert!(c.sleeping);
    assert_eq!(c.r[0], 0);
    c.interrupt(&mut ram, 0x41, 0xE);
    assert!(!c.sleeping);
    assert_eq!(ram.read32(0x7FF8), 0x1002); // returns to the instruction after SLEEP
    run(&mut c, &mut ram, 1);
    assert_eq!(c.r[0], 0x10);
}
