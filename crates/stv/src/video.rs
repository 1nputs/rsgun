//! VDP1 and VDP2 as the CPUs and DMA see them: their memories, their registers, and the status the
//! beam drives. No pixels yet: VDP1's draw walks the command list to its end (so the end-of-draw
//! status and interrupt come as on the chip), and VDP2 composes nothing.

use crate::mem::{merge16, rd16, wr16};
use crate::state::saved;

pub struct Vdp1 {
    pub vram: Vec<u8>,
    /// The two framebuffers; `draw` is the one being drawn into.
    pub fb: [Vec<u8>; 2],
    pub draw: usize,
    pub tvmr: u16,
    pub fbcr: u16,
    pub ptmr: u16,
    pub ewdr: u16,
    pub ewlr: u16,
    pub ewrr: u16,
    pub edsr: u16,
    pub lopr: u16,
    pub copr: u16,
    /// FBCR asked for a manual frame change at the next vertical blank.
    pub change_requested: bool,
}

saved!(Vdp1 { vram, fb, draw, tvmr, fbcr, ptmr, ewdr, ewlr, ewrr, edsr, lopr, copr, change_requested });

impl Default for Vdp1 {
    fn default() -> Vdp1 {
        Vdp1::new()
    }
}

impl Vdp1 {
    pub fn new() -> Vdp1 {
        // Power-on contents as the oracle has them: each 32-byte command slot an end command
        // (0x8000), then 0x5555 and alternating 0xAAAA, 0x5555.
        let mut vram = vec![0; 0x8_0000];
        for (i, w) in vram.as_chunks_mut::<2>().0.iter_mut().enumerate() {
            let v: u16 = if i % 16 == 0 { 0x8000 } else if i % 2 == 1 { 0x5555 } else { 0xAAAA };
            *w = v.to_be_bytes();
        }
        Vdp1 {
            vram,
            fb: [vec![0; 0x4_0000], vec![0; 0x4_0000]],
            draw: 0,
            tvmr: 0,
            fbcr: 0,
            ptmr: 0,
            ewdr: 0,
            ewlr: 0,
            ewrr: 0,
            edsr: 0,
            lopr: 0,
            copr: 0,
            change_requested: false,
        }
    }

    pub fn reset(&mut self) {
        self.tvmr = 0;
        self.fbcr = 0;
        self.ptmr = 0;
        self.edsr = 0;
        self.change_requested = false;
    }

    pub fn read_reg(&self, a: u32) -> u16 {
        match a & 0x1E {
            0x10 => self.edsr,
            0x12 => self.lopr,
            0x14 => self.copr,
            0x16 => 0x1000 | (self.ptmr & 3) << 8 | (self.tvmr & 0xF), // MODR: version 1
            _ => 0,
        }
    }

    /// A register write; returns true when it starts a draw (PTMR = 1).
    pub fn write_reg(&mut self, a: u32, v: u16) -> bool {
        match a & 0x1E {
            0x00 => self.tvmr = v,
            0x02 => {
                self.fbcr = v;
                if v & 3 == 3 {
                    self.change_requested = true;
                }
            }
            0x04 => {
                self.ptmr = v & 3;
                return v & 3 == 1;
            }
            0x06 => self.ewdr = v,
            0x08 => self.ewlr = v,
            0x0A => self.ewrr = v,
            0x0C => {} // ENDR: forced end of a draw that is already over
            _ => {}
        }
        false
    }

    /// Runs a draw: follows the command list from the start of VRAM to its end command. Returns
    /// when it is done (instantly, for now).
    pub fn run_draw(&mut self) {
        let mut addr: usize = 0;
        let mut ret: Option<usize> = None;
        let mut last = 0;
        for _ in 0..0x8000 {
            let ctrl = rd16(&self.vram, addr);
            if ctrl & 0x8000 != 0 {
                break;
            }
            last = addr;
            let link = (rd16(&self.vram, addr + 2) as usize & 0xFFFC) << 3;
            addr = match (ctrl >> 12) & 3 {
                0 => addr + 0x20,
                1 => link,
                2 => {
                    ret = Some(addr + 0x20);
                    link
                }
                _ => ret.take().unwrap_or(addr + 0x20),
            } & 0x7_FFE0;
        }
        self.lopr = (last >> 3) as u16;
        self.copr = (addr >> 3) as u16;
        self.edsr |= 0x02;
    }

    /// The frame change (at VBlank-OUT): the framebuffers swap, the end-of-draw status moves
    /// to "before", and a draw starts in automatic mode. Returns true when a draw started.
    pub fn frame_change(&mut self) -> bool {
        let one_cycle = self.fbcr & 2 == 0;
        if !(one_cycle || self.change_requested) {
            return false;
        }
        self.change_requested = false;
        self.draw ^= 1;
        self.edsr = (self.edsr & 2) >> 1;
        // the framebuffer now drawn into is erased with EWDR over the erase window
        let (x0, y0) = ((((self.ewlr >> 9) & 0x3F) as usize) << 3, (self.ewlr & 0x1FF) as usize);
        let (x1, y1) = ((((self.ewrr >> 9) & 0x7F) as usize) << 3, (self.ewrr & 0x1FF) as usize);
        let fb = &mut self.fb[self.draw];
        for y in y0..=y1.min(255) {
            for x in x0..x1.min(512) {
                fb[(y * 512 + x) * 2..(y * 512 + x) * 2 + 2].copy_from_slice(&self.ewdr.to_be_bytes());
            }
        }
        self.ptmr == 2
    }
}

pub struct Vdp2 {
    pub vram: Vec<u8>,
    pub cram: Vec<u8>,
    /// The registers as written (0x200 bytes), as the oracle's hash takes them.
    pub regs: Vec<u8>,
    pub vblank: bool,
    pub hblank: bool,
    pub odd: bool,
}

saved!(Vdp2 { vram, cram, regs, vblank, hblank, odd });

impl Default for Vdp2 {
    fn default() -> Vdp2 {
        Vdp2::new()
    }
}

impl Vdp2 {
    pub fn new() -> Vdp2 {
        Vdp2 { vram: vec![0; 0x8_0000], cram: vec![0; 0x1000], regs: vec![0; 0x200], vblank: false, hblank: false, odd: true }
    }

    pub fn reset(&mut self) {
        self.regs.fill(0);
    }

    pub fn tvmd(&self) -> u16 {
        rd16(&self.regs, 0)
    }

    /// Lines of display: 224, 240 or 256 (TVMD VRESO).
    pub fn height(&self) -> u32 {
        match (self.tvmd() >> 4) & 3 {
            0 => 224,
            1 => 240,
            _ => 256,
        }
    }

    pub fn read_reg(&mut self, a: u32) -> u16 {
        let a = (a & 0x1FE) as usize;
        match a {
            0x04 => (self.vblank as u16) << 3 | (self.hblank as u16) << 2 | (self.odd as u16) << 1,
            0x06 => 0, // VRSIZE: version 0, 4 Mbit VRAM
            0x08 => 0x2BA,
            _ => rd16(&self.regs, a),
        }
    }

    pub fn write_reg(&mut self, a: u32, v: u16, mask: u16) {
        let a = (a & 0x1FE) as usize;
        let old = rd16(&self.regs, a);
        wr16(&mut self.regs, a, merge16(old, v, mask));
    }
}
