//! Instruction tests from the reference manual's definitions; the oracle's traces check the core on
//! the sound driver itself (stv-check trace, the sound CPU's records).

use super::*;

struct Ram(Vec<u8>);

impl Bus68 for Ram {
    fn read8(&mut self, a: u32) -> u8 {
        self.0[a as usize & 0xFFFF]
    }
    fn read16(&mut self, a: u32) -> u16 {
        u16::from_be_bytes([self.read8(a), self.read8(a + 1)])
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.0[a as usize & 0xFFFF] = v;
    }
    fn write16(&mut self, a: u32, v: u16) {
        self.write8(a, (v >> 8) as u8);
        self.write8(a + 1, v as u8);
    }
    fn fetch16(&mut self, a: u32) -> u16 {
        self.read16(a)
    }
}

/// Code at 0x1000, SSP 0x8000, supervisor mode.
fn setup(code: &[u16]) -> (M68k, Ram) {
    let mut ram = Ram(vec![0; 0x10000]);
    for (i, w) in code.iter().enumerate() {
        ram.write16(0x1000 + 2 * i as u32, *w);
    }
    ram.write16(0, 0);
    ram.write16(2, 0x8000);
    ram.write16(4, 0);
    ram.write16(6, 0x1000);
    let mut c = M68k::default();
    c.reset(&mut ram);
    (c, ram)
}

fn run(c: &mut M68k, ram: &mut Ram, n: usize) {
    for _ in 0..n {
        c.step(ram);
    }
}

#[test]
fn moveq_add_sub_flags() {
    // moveq #-1,d0; moveq #1,d1; add.b d0,d1 (0xFF + 0x01 = 0, carry); sub.w d1,d0
    let (mut c, mut ram) = setup(&[0x70FF, 0x7201, 0xD200, 0x9041]);
    run(&mut c, &mut ram, 3);
    assert_eq!(c.d[1], 0);
    assert_eq!(c.sr & 0x1F, SR_X | SR_Z | SR_C);
    run(&mut c, &mut ram, 1);
    assert_eq!(c.d[0], 0xFFFF_FFFF); // 0xFFFF - 0 = 0xFFFF, low word only
    assert_eq!(c.sr & 0x1F, SR_N);
}

#[test]
fn add_overflow_and_cmp() {
    // move.l #0x7FFFFFFF,d0; addq.l #1,d0; cmp.l #0x80000000,d0
    let (mut c, mut ram) = setup(&[0x203C, 0x7FFF, 0xFFFF, 0x5280, 0xB0BC, 0x8000, 0x0000]);
    run(&mut c, &mut ram, 2);
    assert_eq!(c.d[0], 0x8000_0000);
    assert_eq!(c.sr & 0x1F, SR_N | SR_V);
    run(&mut c, &mut ram, 1);
    assert_eq!(c.sr & 0x0F, SR_Z);
}

#[test]
fn bcd_add_and_subtract() {
    // move #0x10,ccr... use: moveq #0x19,d0; moveq #0x28,d1; andi #0,ccr; abcd d1,d0 -> 0x47
    let (mut c, mut ram) = setup(&[0x7019, 0x7228, 0x023C, 0x0000, 0xC101, 0x7047, 0x7228, 0x023C, 0x0000, 0x8101]);
    run(&mut c, &mut ram, 4);
    assert_eq!(c.d[0] & 0xFF, 0x47);
    run(&mut c, &mut ram, 4);
    assert_eq!(c.d[0] & 0xFF, 0x19); // 0x47 - 0x28 = 0x19
}

#[test]
fn divide_and_multiply() {
    // move.l #100003,d0; divu #10,d0 -> q 10000 r 3; muls #-3,d1 with d1 = 7
    let (mut c, mut ram) = setup(&[0x203C, 0x0001, 0x86A3, 0x80FC, 0x000A, 0x7207, 0xC3FC, 0xFFFD]);
    run(&mut c, &mut ram, 2);
    assert_eq!(c.d[0], (3 << 16) | 10000);
    run(&mut c, &mut ram, 2);
    assert_eq!(c.d[1], (-21i32) as u32);
    assert!(c.sr & SR_N != 0);
}

#[test]
fn shifts_and_rotates() {
    // moveq #-127,d0 (0xFFFFFF81); asl.b #1,d0 -> 0x02, C X V; roxr.b #1,d0 with X=1 -> 0x81
    let (mut c, mut ram) = setup(&[0x7081, 0xE300, 0xE210]);
    run(&mut c, &mut ram, 2);
    assert_eq!(c.d[0] & 0xFF, 0x02);
    assert_eq!(c.sr & 0x1F, SR_X | SR_V | SR_C);
    run(&mut c, &mut ram, 1);
    assert_eq!(c.d[0] & 0xFF, 0x81);
    assert_eq!(c.sr & 0x1F, SR_N);
}

#[test]
fn movem_and_stack() {
    // moveq #1,d0; moveq #2,d1; movem.l d0-d1,-(sp); movem.l (sp)+,d2-d3
    let (mut c, mut ram) = setup(&[0x7001, 0x7202, 0x48E7, 0xC000, 0x4CDF, 0x000C]);
    run(&mut c, &mut ram, 3);
    assert_eq!(c.a[7], 0x7FF8);
    assert_eq!((ram.read16(0x7FF8), ram.read16(0x7FFA), ram.read16(0x7FFC), ram.read16(0x7FFE)), (0, 1, 0, 2));
    run(&mut c, &mut ram, 1);
    assert_eq!((c.d[2], c.d[3], c.a[7]), (1, 2, 0x8000));
}

#[test]
fn branches_and_subroutines() {
    // 1000: bsr.s +4 (to 1006); 1002: bra.s 1002 ...; 1006: moveq #5,d0; rts
    // then dbf: moveq #2,d1; loop: addq #1,d2; dbf d1,loop
    let (mut c, mut ram) = setup(&[0x6104, 0x6000, 0x0006, 0x7005, 0x4E75, 0x7202, 0x5242, 0x51C9, 0xFFFC]);
    run(&mut c, &mut ram, 3);
    assert_eq!((c.pc, c.d[0]), (0x1002, 5));
    run(&mut c, &mut ram, 1); // bra.w +6 -> 0x100A
    assert_eq!(c.pc, 0x100A);
    run(&mut c, &mut ram, 1 + 3 * 2);
    assert_eq!((c.d[2], c.d[1] & 0xFFFF), (3, 0xFFFF));
}

#[test]
fn trap_rte_and_interrupts() {
    // trap #3 (vector 35) -> handler at 0x1100: moveq #9,d0; rte
    let (mut c, mut ram) = setup(&[0x4E43, 0x7001]);
    ram.write16(35 * 4, 0);
    ram.write16(35 * 4 + 2, 0x1100);
    ram.write16(0x1100, 0x7009);
    ram.write16(0x1102, 0x4E73);
    run(&mut c, &mut ram, 1);
    assert_eq!((c.pc, c.a[7]), (0x1100, 0x7FFA));
    run(&mut c, &mut ram, 2);
    assert_eq!((c.pc, c.a[7], c.d[0]), (0x1002, 0x8000, 9));
    // a level-2 autovectored interrupt while the mask is 0
    c.sr = 0x2000;
    ram.write16((24 + 2) * 4, 0);
    ram.write16((24 + 2) * 4 + 2, 0x1100);
    c.ipl = 2;
    run(&mut c, &mut ram, 1);
    assert_eq!((c.pc, (c.sr >> 8) & 7), (0x1100, 2));
}
