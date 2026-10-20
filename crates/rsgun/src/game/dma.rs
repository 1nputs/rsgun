//! dma.c: the copies to video memory the game queues during a frame and makes at its end
//! (dma_control, from the main loop). The cart's leftover dma.c object names it all: its functions,
//! linked at 0x060044a0, and its work area `dma_wk`, with the types its debug information gives.
//! Long copies are queued apart and made a piece a frame.

use super::nested;
use super::Cpu;

/// `DMA_WK dma_wk`: the queue (count, each copy's SGL mode, source, destination, size) and the long
/// copies' (count, source, destination, size still to go).
const DMA_WK: u32 = 0x0600_a9b8;
const DMA_SUU: u32 = DMA_WK; // char
const DIV_DMA_SUU: u32 = DMA_WK + 0x01; // char
const MODE: u32 = DMA_WK + 0x02; // char[8]
const SRC: u32 = DMA_WK + 0x0c; // void *[8]
const DST: u32 = DMA_WK + 0x2c; // void *[8]
const SIZE: u32 = DMA_WK + 0x4c; // unsigned long[8]
const DSRC: u32 = DMA_WK + 0x6c; // void *[8]
const DDST: u32 = DMA_WK + 0x8c; // void *[8]
const DSIZE: u32 = DMA_WK + 0xac; // unsigned long[8]

/// How many copies each queue holds.
const QUEUE: i8 = 8;
/// A long copy goes a piece a frame: the most it copies at once, and the piece when it is more.
const PIECE_MAX: u32 = 0x3000;
const PIECE: u32 = 0x2000;

/// SGL's copies (CPU DMA): slDMACopy(src, dst, size), slDMAXCopy(src, dst, size, mode).
const SL_DMA_COPY: u32 = 0x0603_3a10;
const SL_DMAX_COPY: u32 = 0x0603_3aa0;

/// dma_set: queues a copy for the frame's end. The copies queued then, or 0 if the queue is full.
pub fn dma_set(cpu: &mut Cpu, src: u32, dst: u32, size: u32) -> u32 {
    queue(cpu, src, dst, size, 0)
}

/// mode_dma_set: dma_set with SGL's copy mode (slDMAXCopy's).
pub fn mode_dma_set(cpu: &mut Cpu, src: u32, dst: u32, size: u32, mode: u32) -> u32 {
    queue(cpu, src, dst, size, mode as i8)
}

fn queue(cpu: &mut Cpu, src: u32, dst: u32, size: u32, mode: i8) -> u32 {
    let n = cpu.read8(DMA_SUU) as i8;
    if n >= QUEUE {
        return 0;
    }
    let i = n as u32;
    cpu.write32(SRC + 4 * i, src);
    cpu.write32(DST + 4 * i, dst);
    cpu.write32(SIZE + 4 * i, size);
    cpu.write8(MODE + i, mode as u8);
    cpu.write8(DMA_SUU, (n + 1) as u8);
    (n + 1) as i32 as u32
}

/// divdma_set: queues a long copy, made a piece a frame. The long copies queued then, or 0 if
/// their queue is full.
pub fn divdma_set(cpu: &mut Cpu, src: u32, dst: u32, size: u32) -> u32 {
    let n = cpu.read8(DIV_DMA_SUU) as i8;
    if n >= QUEUE {
        return 0;
    }
    let i = n as u32;
    cpu.write32(DSRC + 4 * i, src);
    cpu.write32(DDST + 4 * i, dst);
    cpu.write32(DSIZE + 4 * i, size);
    cpu.write8(DIV_DMA_SUU, (n + 1) as u8);
    (n + 1) as i32 as u32
}

/// dma_control: at the frame's end, queues the next piece of the first long copy, then makes the
/// copies queued (the last first).
pub fn dma_control(cpu: &mut Cpu) {
    let long_copies = cpu.read8(DIV_DMA_SUU) as i8;
    if long_copies != 0 {
        let (src, dst, left) = (cpu.read32(DSRC), cpu.read32(DDST), cpu.read32(DSIZE));
        let piece = if left > PIECE_MAX { PIECE } else { left };
        if nested!(cpu, 0x0600_44a0, dma_set(cpu, src, dst, piece)) != 0 {
            if piece < left {
                cpu.write32(DSRC, src.wrapping_add(piece));
                cpu.write32(DSIZE, left - piece);
                cpu.write32(DDST, dst.wrapping_add(piece));
            } else {
                // done: the others move up
                let n = long_copies - 1;
                cpu.write8(DIV_DMA_SUU, n as u8);
                for i in 0..n.max(0) as u32 {
                    for list in [DSRC, DDST, DSIZE] {
                        let next = cpu.read32(list + 4 * (i + 1));
                        cpu.write32(list + 4 * i, next);
                    }
                }
            }
        }
    }
    loop {
        let n = cpu.read8(DMA_SUU) as i8;
        if n <= 0 {
            break;
        }
        let i = (n - 1) as u32;
        cpu.write8(DMA_SUU, i as u8);
        let (src, dst, size) = (cpu.read32(SRC + 4 * i), cpu.read32(DST + 4 * i), cpu.read32(SIZE + 4 * i));
        match cpu.read8(MODE + i) as i8 {
            0 => cpu.call(SL_DMA_COPY, &[src, dst, size]),
            mode => cpu.call(SL_DMAX_COPY, &[src, dst, size, mode as i32 as u32]),
        };
    }
}

/// dma_control_init: empties both queues.
pub fn dma_control_init(cpu: &mut Cpu) {
    cpu.write8(DMA_SUU, 0);
    cpu.write8(DIV_DMA_SUU, 0);
}

/// dma_check: whether copies are queued (either queue).
pub fn dma_check(cpu: &mut Cpu) -> u32 {
    let queued = cpu.read8(DMA_SUU) != 0 || cpu.read8(DIV_DMA_SUU) != 0;
    queued as u32
}
