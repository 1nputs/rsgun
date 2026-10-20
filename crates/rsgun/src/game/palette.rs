//! The colours: the game keeps VDP2's colour RAM and a table of VDP1's colours (in its VRAM) as
//! images in work RAM, each beside a copy of the colours as they were set. A frame that changes an
//! image has it copied to a staging buffer at its end, and the next frame's VBlank sends the
//! staging buffer to the video chip (SGL's transfer list), so the colours change with the picture
//! drawn with them.

use super::Cpu;

/// What changed this frame (bit 0 colour RAM, bit 1 VDP1's table) and what is staged for this
/// VBlank (bit 2, bit 3).
const CHANGED: u32 = 0x0603_0e30;
const CRAM_CHANGED: u32 = 1 << 0;
const VDP1_CHANGED: u32 = 1 << 1;
const CRAM_STAGED: u32 = 1 << 2;
const VDP1_STAGED: u32 = 1 << 3;

/// Colour RAM's colours as set, its image, its staging buffer, and where it goes.
const CRAM_SET: u32 = 0x0602_fe30;
const CRAM_IMAGE: u32 = 0x0603_0e40;
const CRAM_STAGE: u32 = 0x0600_aa94;
const CRAM: u32 = 0x25f0_0000;
/// The same for VDP1's colour table (in its VRAM).
const VDP1_SET: u32 = 0x0602_ee30;
const VDP1_IMAGE: u32 = 0x0603_1e40;
const VDP1_STAGE: u32 = 0x0600_ba94;
const VDP1_TABLE: u32 = 0x25c0_e000;
/// Each image's size.
const IMAGE: u32 = 0x1000;

/// SGL: slTransferEntry(src, dst, size), a copy for the VBlank; slDMACopy(src, dst, size).
const SL_TRANSFER_ENTRY: u32 = 0x0603_4a50;
const SL_DMA_COPY: u32 = 0x0603_3a10;

/// Sets `count` colours of colour RAM from `index` on, from `colours`.
pub fn set_colours(cpu: &mut Cpu, colours: u32, index: u32, count: u32) {
    let at = 2 * (index & 0xFFFF);
    for i in 0..count & 0xFFFF {
        let c = cpu.read16(colours + 2 * i);
        cpu.write16(CRAM_SET + at + 2 * i, c);
        cpu.write16(CRAM_IMAGE + at + 2 * i, c);
    }
    let changed = cpu.read32(CHANGED);
    cpu.write32(CHANGED, changed | CRAM_CHANGED);
}

/// Sets `count` colours of VDP1's table from `index` on, from `colours` (as RGB: bit 15 set).
pub fn set_vdp1_colours(cpu: &mut Cpu, colours: u32, index: u32, count: u32) {
    let at = 2 * (index & 0xFFFF);
    for i in 0..count & 0xFFFF {
        let c = cpu.read16(colours + 2 * i) | 0x8000;
        cpu.write16(VDP1_SET + at + 2 * i, c);
        cpu.write16(VDP1_IMAGE + at + 2 * i, c);
    }
    let changed = cpu.read32(CHANGED);
    cpu.write32(CHANGED, changed | VDP1_CHANGED);
}

/// At the frame's end: what was staged goes to the video chips at this VBlank, and what changed is
/// staged for the next.
pub fn send_colours(cpu: &mut Cpu) {
    let changed = cpu.read32(CHANGED);
    if changed & CRAM_STAGED != 0 {
        cpu.call(SL_TRANSFER_ENTRY, &[CRAM_STAGE, CRAM, IMAGE]);
    }
    if changed & VDP1_STAGED != 0 {
        cpu.call(SL_TRANSFER_ENTRY, &[VDP1_STAGE, VDP1_TABLE, IMAGE]);
    }
    let mut now = changed & !(CRAM_STAGED | VDP1_STAGED);
    if changed & CRAM_CHANGED != 0 {
        cpu.call(SL_DMA_COPY, &[CRAM_IMAGE, CRAM_STAGE, IMAGE]);
        now |= CRAM_STAGED;
    }
    if changed & VDP1_CHANGED != 0 {
        cpu.call(SL_DMA_COPY, &[VDP1_IMAGE, VDP1_STAGE, IMAGE]);
        now |= VDP1_STAGED;
    }
    cpu.write32(CHANGED, now & !(CRAM_CHANGED | VDP1_CHANGED));
}
