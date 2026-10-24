//! The game's interrupt layer (SGL's, in the main program): the handlers the game installs for the
//! SH-2's interrupt vectors, kept in a table of its own (HANDLERS) beside the BIOS's, the SR's
//! interrupt mask raised while they are changed, and the entries the BIOS's exception stubs
//! (bios.rs) call, which look the handler up and run it with MACH, MACL and PR kept.

use super::{nested, Cpu};
use stv::sh2::SR_MASK;

/// The handlers the game has installed, a word for each vector (0: none), and the table the
/// vector stubs look their handler up in.
const HANDLERS: u32 = 0x0607_6a80;
const DISPATCH: u32 = 0x0607_6b80;
/// The stubs the game gives the BIOS for each vector (a word each: the stub for vector n is
/// 0x0606520c + 6 n for n below 14 and 6 (n - 2) from 16 on; these call the handler in DISPATCH).
const STUBS: u32 = 0x0606_4ec4;
/// The BIOS's function, by its table, that sets the handler of a vector.
const BIOS_SET_INTERRUPT: u32 = 0x0600_0300;
/// SR's interrupt mask.
const SR_LEVEL: u32 = 0xf0;

/// 0x06064f68: the handler for `vector` set to `handler` (0: the BIOS's default).
pub fn set_handler(cpu: &mut Cpu, vector: u32, handler: u32) {
    let frame = cpu.stack();
    cpu.write32(frame, vector);
    cpu.write32(frame + 4, handler);
    nested!(cpu, 0x0606_5064, returning 0x0606_4f82, install(cpu, vector, handler));
}

/// 0x06065064: `handler` kept for `vector` in HANDLERS and the vector given to the BIOS (the game's
/// stub for it from STUBS, or the BIOS's default for 0), the interrupt mask raised to 15 meanwhile.
pub fn install(cpu: &mut Cpu, vector: u32, handler: u32) {
    let frame = cpu.stack();
    cpu.write32(frame, vector);
    cpu.write32(frame + 4, handler);
    let level = nested!(cpu, 0x0606_515c, returning 0x0606_5078, mask_level(cpu));
    cpu.write32(frame + 12, level);
    nested!(cpu, 0x0606_5194, returning 0x0606_5082, set_mask_level(cpu, 15));
    cpu.write32(HANDLERS + 4 * vector, handler);
    let stub = if handler == 0 { 0 } else { cpu.read32(STUBS + 4 * vector) };
    cpu.write32(frame + 8, stub);
    let set = cpu.read32(BIOS_SET_INTERRUPT);
    cpu.call_returning(0x0606_50be, set, &[vector, stub]);
    nested!(cpu, 0x0606_5194, returning 0x0606_50c8, set_mask_level(cpu, level));
}

/// 0x0606515c: the interrupt mask the CPU runs at now (SR's bits 4-7).
pub fn mask_level(cpu: &mut Cpu) -> u32 {
    let frame = cpu.stack();
    let sr = nested!(cpu, 0x0606_51f0, returning 0x0606_516c, get_sr(cpu));
    let level = (sr & SR_LEVEL) >> 4;
    cpu.write32(frame, level);
    level
}

/// 0x06065194: the interrupt mask set to `level` (SR's other bits kept).
pub fn set_mask_level(cpu: &mut Cpu, level: u32) {
    let frame = cpu.stack();
    let sr = nested!(cpu, 0x0606_51f0, returning 0x0606_51a8, get_sr(cpu));
    cpu.write32(frame, sr);
    let kept = sr & 0xffff_ff0f;
    cpu.write32(frame, kept);
    let value = level << 4 | kept;
    cpu.write32(frame, value);
    nested!(cpu, 0x0606_51e0, returning 0x0606_51c6, set_sr(cpu, value));
}

/// 0x060651e0: SR set to `value` (interrupts due at the new mask are taken after it).
pub fn set_sr(cpu: &mut Cpu, value: u32) {
    cpu.sh2.sr = value & SR_MASK;
}

/// 0x060651f0: SR as it is.
pub fn get_sr(cpu: &mut Cpu) -> u32 {
    let sr = cpu.sh2.sr;
    let frame = cpu.stack();
    cpu.write32(frame, sr);
    sr
}

/// The vector stubs (0x0606520c ...: each `r0 = vector; bra 0x060652c0`) and their common code at
/// 0x060652c0: the handler in DISPATCH for the vector, if there is one, called with PR, MACH and
/// MACL kept on the stack.
fn vector_stub(cpu: &mut Cpu, vector: u32) {
    let macl = cpu.sh2.macl;
    cpu.push(&[macl]);
    let handler = cpu.read32(DISPATCH + 4 * vector);
    if handler != 0 {
        let (pr, mach) = (cpu.pr(), cpu.sh2.mach);
        cpu.push(&[pr]);
        cpu.push(&[mach]);
        cpu.call_returning(0x0606_52d0, handler, &[]);
        cpu.sh2.mach = cpu.read32(cpu.sh2.r[15]);
        cpu.pop(4);
        cpu.sh2.pr = cpu.read32(cpu.sh2.r[15]);
        cpu.pop(4);
    }
    cpu.sh2.macl = cpu.read32(cpu.sh2.r[15]);
    cpu.pop(4);
}

/// 0x0606521e: the stub of vector 3.
pub fn vector_3(cpu: &mut Cpu) {
    vector_stub(cpu, 3);
}
