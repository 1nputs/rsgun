//! The VBlank callbacks: up to four functions the game has SGL call at each VBlank (the first, the
//! VBlank count).

use super::Cpu;

/// How many are set, and the list.
const COUNT: u32 = 0x0600_a9a0;
const LIST: u32 = 0x0600_a9a8;
const MAX: u32 = 4;

/// SGL: slIntFunction(f), the function called at each VBlank.
const SL_INT_FUNCTION: u32 = 0x0603_3a00;
/// This module's run_callbacks and system::count_vblank, at their SH-2 entries.
const RUN_CALLBACKS: u32 = 0x0600_438c;
const COUNT_VBLANK: u32 = 0x0600_4378;

/// At the game's start: no callbacks but the VBlank count, run from SGL's VBlank.
pub fn init_vblank(cpu: &mut Cpu) {
    cpu.write32(COUNT, 0);
    cpu.call(SL_INT_FUNCTION, &[RUN_CALLBACKS]);
    add_callback(cpu, COUNT_VBLANK);
}

/// At each VBlank: each callback, in the order they were set.
pub fn run_callbacks(cpu: &mut Cpu) {
    let mut i = 0;
    while (i as i32) < cpu.read32(COUNT) as i32 {
        let f = cpu.read32(LIST + 4 * i);
        cpu.call(f, &[]);
        i += 1;
    }
}

/// Adds `f` to the callbacks: how many there were, or -1 if there is no room.
pub fn add_callback(cpu: &mut Cpu, f: u32) -> u32 {
    let n = cpu.read32(COUNT) as i32;
    if n >= MAX as i32 {
        return u32::MAX;
    }
    cpu.write32(LIST + 4 * n as u32, f);
    cpu.write32(COUNT, (n + 1) as u32);
    n as u32
}

/// Takes `f` out of the callbacks (the ones after it move up): 1, or -1 if it was not there.
pub fn remove_callback(cpu: &mut Cpu, f: u32) -> u32 {
    let Some(i) = (0..MAX).find(|&i| cpu.read32(LIST + 4 * i) == f) else {
        return u32::MAX;
    };
    for j in i..MAX - 1 {
        let next = cpu.read32(LIST + 4 * (j + 1));
        cpu.write32(LIST + 4 * j, next);
    }
    let n = cpu.read32(COUNT);
    cpu.write32(COUNT, n.wrapping_sub(1));
    1
}
