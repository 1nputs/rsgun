//! Small pieces of the game's system: the VBlank count, coins and credits as the game keeps them,
//! the SH-2's cache and DMA, the interrupt mask.

use super::Cpu;
use super::globals::VBLANKS;

/// The coin and credit state, a 4-byte entry for each coin slot (player), kept by the BIOS: +0 the
/// credits, +1 the coins toward the next one, +2 the coins a credit takes, +3 (read by the title
/// and the attract mode before they offer a start; not yet known). Then the settings: free play (bit 0), a coin slot for each player
/// (bit 2), and a byte whose low half the credit display reads.
const COINS: u32 = 0x0600_ebc0;
const COIN_SETTINGS: u32 = 0x0600_ebd0;
const FREE_PLAY: u8 = 1 << 0;
const SLOT_EACH: u8 = 1 << 2;

/// The SH-2's cache control register, and DMA channel 0's control register.
const CCR: u32 = 0xFFFF_FE92;
const CHCR0: u32 = 0xFFFF_FF8C;

/// count_vblank (a VBlank callback): one more VBlank.
pub fn count_vblank(cpu: &mut Cpu) {
    let n = cpu.read32(VBLANKS);
    cpu.write32(VBLANKS, n.wrapping_add(1));
}

/// credit_ct (the leftover map's name): the credits for `slot`'s player; 0 on free play, slot 0's
/// when the slots share them.
pub fn credits(cpu: &mut Cpu, slot: u32) -> u32 {
    let settings = cpu.read8(COIN_SETTINGS);
    if settings & FREE_PLAY != 0 {
        return 0;
    }
    let slot = if settings & SLOT_EACH != 0 { slot } else { 0 };
    cpu.read8(COINS + 4 * slot) as u32
}

/// Whether the game is on free play (the coin settings).
pub fn free_play(cpu: &mut Cpu) -> bool {
    cpu.read8(COIN_SETTINGS) & FREE_PLAY != 0
}

/// The coin settings as the credit display takes them: on free play, never a slot each.
pub fn coin_settings_shown(cpu: &mut Cpu) -> u32 {
    let settings = cpu.read8(COIN_SETTINGS);
    if cpu.read8(COIN_SETTINGS) & FREE_PLAY != 0 {
        return (settings & !SLOT_EACH) as u32;
    }
    settings as u32
}

/// The coins toward `slot`'s next credit (slot 0's when the slots share them).
pub fn coins(cpu: &mut Cpu, slot: u32) -> u32 {
    let slot = if cpu.read8(COIN_SETTINGS) & SLOT_EACH != 0 { slot } else { 0 };
    cpu.read8(COINS + 4 * slot + 1) as u32
}

/// The coins a credit takes for `slot` (slot 0's when the slots share them).
pub fn coins_per_credit(cpu: &mut Cpu, slot: u32) -> u32 {
    let slot = if cpu.read8(COIN_SETTINGS) & SLOT_EACH != 0 { slot } else { 0 };
    cpu.read8(COINS + 4 * slot + 2) as u32
}

/// The entry's byte +3 for `slot`: as the title reads it, 0 when its player has nothing to start
/// with, 1 when coins are in but short of a credit (as far as can be told), 2 when the player can
/// start.
pub fn coin_byte3(cpu: &mut Cpu, slot: u32) -> u32 {
    cpu.read8(COINS + 4 * slot + 3) as u32
}

/// Calls the BIOS's function whose address is at `function` (its table).
pub fn bios(cpu: &mut Cpu, function: u32, args: &[u32]) -> u32 {
    let f = cpu.read32(function);
    cpu.call(f, args)
}

/// The BIOS's function that takes a credit for a player.
const BIOS_USE_CREDIT: u32 = 0x0600_0664;

/// A credit taken for `slot`'s player: nonzero if there was one.
pub fn use_credit(cpu: &mut Cpu, slot: u32) -> u32 {
    bios(cpu, BIOS_USE_CREDIT, &[slot])
}

/// The settings byte's low half the credit display reads.
pub fn coin_settings_low(cpu: &mut Cpu) -> u32 {
    (cpu.read8(COIN_SETTINGS + 1) & 0xF) as u32
}

/// SGL's cache purge: the cache's entries are invalidated (CCR: purge, enable).
pub fn purge_cache(cpu: &mut Cpu) {
    cpu.write8(CCR, 0x11);
}

/// Waits while DMA channel 0 is copying (enabled, not yet ended).
pub fn wait_dma0(cpu: &mut Cpu) {
    while cpu.read32(CHCR0) & 3 == 1 {
        cpu.wait(0x0606_e24e, 4);
    }
}

/// SGL's set_imask: the CPU takes interrupts above `mask` (0-15).
pub fn set_interrupt_mask(cpu: &mut Cpu, mask: u32) {
    cpu.sh2.sr = (cpu.sh2.sr & !0xF0) | (mask & 0xF) << 4;
}

/// The BIOS's: `n + 1`, but -1 stays -1.
pub fn next_unless_end(_cpu: &mut Cpu, n: u32) -> u32 {
    if n == u32::MAX { n } else { n.wrapping_add(1) }
}

/// The SCU's interrupt mask, as the BIOS keeps a copy of it, and its A-bus acknowledge.
const SCU_MASK_COPY: u32 = 0x0600_0348;
const IMS: u32 = 0x25fe_00a0;
const AIACK: u32 = 0x25fe_00a8;

/// The SCU's interrupt mask: kept where `mask` has bits, and `bits` set (interrupts held off
/// meanwhile); the A-bus interrupts acknowledged unless bit 15 is in either.
pub fn set_scu_mask(cpu: &mut Cpu, mask: u32, bits: u32) {
    let sr = cpu.sh2.sr;
    cpu.sh2.sr |= 0xF0;
    let m = cpu.read32(SCU_MASK_COPY) & mask | bits;
    cpu.write32(SCU_MASK_COPY, m);
    cpu.write32(IMS, m);
    if ((mask | bits) as i16) >= 0 {
        cpu.write32(AIACK, 1);
    }
    cpu.sh2.sr = sr;
}

/// `x` shifted left by `bits` & 31 (the game's helper does it a bit at a time: by 1, 2, 4, 8, 16).
pub fn shift_left(_cpu: &mut Cpu, x: u32, bits: u32) -> u32 {
    x << (bits & 0x1F)
}
