//! Small pieces of the game's system: the VBlank count, coins and credits as the game keeps them,
//! the SH-2's cache and DMA, the interrupt mask.

use super::Cpu;

/// VBlanks counted since the game started (count_vblank, from the VBlank callbacks); the main
/// loop measures its frames by it.
const VBLANKS: u32 = 0x0600_ebb4;

/// The coin and credit state, a 4-byte entry for each coin slot (player): +0 the credits, +1 the
/// coins toward the next one, +3 (read by the title and the attract mode before they offer a
/// start; not yet known). Then the settings: free play (bit 0), a coin slot for each player
/// (bit 2), and a byte whose low half the credit display reads.
const COINS: u32 = 0x0600_ebc0;
const COIN_SETTINGS: u32 = 0x0600_ebd0;
const FREE_PLAY: u8 = 1 << 0;
const SLOT_EACH: u8 = 1 << 2;

/// Sound RAM: the sound driver's command slots (16 bytes each; the first byte is 0 when the
/// driver has done the slot's command).
const SOUND_COMMANDS: u32 = 0x25a0_0700;

/// The SH-2's cache control register, and DMA channel 0's control register.
const CCR: u32 = 0xFFFF_FE92;
const CHCR0: u32 = 0xFFFF_FF8C;

/// count_vblank (a VBlank callback): one more VBlank.
pub fn count_vblank(cpu: &mut Cpu) {
    let n = cpu.read32(VBLANKS);
    cpu.write32(VBLANKS, n.wrapping_add(1));
}

/// Whether the sound driver has done the command in `slot` (1) or not yet (0).
pub fn sound_command_done(cpu: &mut Cpu, slot: u32) -> u32 {
    (cpu.read8(SOUND_COMMANDS + 16 * slot) == 0) as u32
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

/// The entry's byte +3 for `slot`.
pub fn coin_byte3(cpu: &mut Cpu, slot: u32) -> u32 {
    cpu.read8(COINS + 4 * slot + 3) as u32
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
