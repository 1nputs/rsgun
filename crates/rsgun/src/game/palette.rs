//! The colours: the game keeps VDP2's colour RAM and a table of VDP1's colours (in its VRAM) as
//! images in work RAM, each beside a copy of the colours as they were set. A frame that changes an
//! image has it copied to a staging buffer at its end, and the next frame's VBlank sends the
//! staging buffer to the video chip (SGL's transfer list), so the colours change with the picture
//! drawn with them.

use super::{nested, Cpu};

/// What changed this frame (bit 0 colour RAM, bit 1 VDP1's table) and what is staged for this
/// VBlank (bit 2, bit 3).
pub const CHANGED: u32 = 0x0603_0e30;
const CRAM_CHANGED: u32 = 1 << 0;
const VDP1_CHANGED: u32 = 1 << 1;
pub const CRAM_STAGED: u32 = 1 << 2;
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

/// A blend of colours (the struct at `b`: how far (a word, 0-0x1000), the count (a word), the
/// colours from, to and the result's place): each channel moved that far from the first colour
/// toward the second (bit 15 the first's); colour RAM and VDP1's table both changed.
const B_HOW_FAR: u32 = 0;
const B_COUNT: u32 = 2;
const B_FROM: u32 = 4;
const B_TO: u32 = 8;
const B_OUT: u32 = 0xc;
const CHANNELS: [u16; 3] = [0x1f, 0x3e0, 0x7c00];
const RGB_BIT: u16 = 0x8000;

pub fn colours_blend(cpu: &mut Cpu, b: u32) {
    let f = cpu.stack();
    cpu.write32(f, b);
    let t = ((cpu.read16(b + B_HOW_FAR) as i16 as i32) << 4).clamp(0, 0x1_0000) as u32;
    cpu.write32(f + 4, t);
    let mut i = 0;
    while i < cpu.read16(b + B_COUNT) as i16 as i32 {
        let at = 2 * i as u32;
        let (from, to) = (cpu.read32(b + B_FROM), cpu.read32(b + B_TO));
        let (from, to) = (cpu.read16(from + at), cpu.read16(to + at));
        let mut c = from & RGB_BIT;
        for mask in CHANNELS {
            let (a, z) = ((from & mask) as u32, (to & mask) as u32);
            let step = nested!(cpu, 0x0600_6a58, super::math::fixed_mul(cpu, z.wrapping_sub(a), t));
            c = c.wrapping_add((step.wrapping_add(a) as u16) & mask);
        }
        let out = cpu.read32(b + B_OUT);
        cpu.write16(out + at, c);
        i += 1;
    }
    let changed = cpu.read32(CHANGED);
    cpu.write32(CHANGED, changed | CRAM_CHANGED | VDP1_CHANGED);
}

/// Colours shifted (the struct at `s`: red, green and blue offsets, words whose bits 3-8 are an
/// offset of 6 bits; the count, a word; the colours from and the result's place): each channel
/// moved by its offset, one that passes its top or bottom held there (the top for an offset not
/// below 0, else 0; bit 15 the colour's); colour RAM and VDP1's table both changed.
const S_COUNT: u32 = 6;
const S_FROM: u32 = 8;
const S_OUT: u32 = 0xc;
const OFFSET_BITS: u16 = 0x1f8;

pub fn colours_shift(cpu: &mut Cpu, s: u32) {
    let f = cpu.stack();
    let [red, green, blue] = [0, 1, 2].map(|k| cpu.read16(s + 2 * k) as i16);
    let held = |o: i16, top: u16| if o >= 0 { top } else { 0 } as u32;
    let (red_held, green_held, blue_held) = (held(red, 0x1f), held(green, 0x3e0), held(blue, 0x7c00));
    cpu.write32(f, red_held);
    cpu.write32(f + 4, blue_held);
    let red = (red as u16 & OFFSET_BITS) as u32 >> 3;
    let green = ((green as u16 & OFFSET_BITS) as u32) << 2;
    let blue = ((blue as u16 & OFFSET_BITS) << 7) as i16 as i32 as u32;
    let count = cpu.read16(s + S_COUNT) as i16 as i32;
    let (from, out) = (cpu.read32(s + S_FROM), cpu.read32(s + S_OUT));
    for i in 0..count.max(0) as u32 {
        let c = cpu.read16(from + 2 * i) as u32;
        let mut v = c & RGB_BIT as u32;
        let r = (c & 0x1f) + red;
        v += if r & 0x20 != 0 { red_held } else { r & 0x1f };
        let g = (c & 0x3e0) + green;
        v += if g >> 10 & 1 != 0 { green_held } else { g & 0x3e0 };
        let b = (c & 0x7c00).wrapping_add(blue) & 0xffff;
        v += if b & 0x8000 != 0 { blue_held } else { b & 0x7c00 };
        cpu.write16(out + 2 * i, v as u16);
    }
    let changed = cpu.read32(CHANGED);
    cpu.write32(CHANGED, changed | CRAM_CHANGED | VDP1_CHANGED);
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
