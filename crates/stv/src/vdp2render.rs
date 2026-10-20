//! VDP2's composition: the picture a frame shows, from VDP2's registers, VRAM and CRAM and VDP1's
//! framebuffer. From the VDP2 user's manual; checked against the oracle's pictures of its own state
//! (stv-check vdp2).
//!
//! Covered: NBG0-3 in cell mode (one- and two-word pattern names, 1x1 and 2x2 characters, plane maps,
//! 16 and 256 colours, flips), NBG0/1 scroll with fractions and zoom, the sprite layer (types 0-7,
//! palette or RGB), priorities (special priority per character), colour calculation (ratio and add,
//! per screen, per character, by CRAM's MSB), colour offset, the back screen, RBG0 with rotation
//! parameter A and its coefficient table. Not yet: bitmaps, parameter B and its switching, windows,
//! line and vertical cell scroll, mosaic, shadow, the line colour screen, 2048 colours and RGB cells,
//! 8-bit framebuffers. Hi-res (640/704 dots) and double-density interlace (both fields at once).

use crate::mem::rd16;

/// One layer's pixel: its colour (8 bits a channel), priority, and how it takes part in colour
/// calculation and colour offset.
#[derive(Clone, Copy, Debug, Default)]
struct Px {
    rgb: [u8; 3],
    prio: u8,
    cc: bool,
    ratio: u8,
    offset: Option<usize>,
}


pub struct Vdp2Frame<'a> {
    pub regs: &'a [u8],
    pub vram: &'a [u8],
    pub cram: &'a [u8],
    /// VDP1's framebuffer on display (512 x 256, 16-bit pixels) and its TVMR.
    pub fb: &'a [u8],
    pub tvmr: u8,
}

impl Vdp2Frame<'_> {
    fn reg(&self, off: usize) -> u16 {
        rd16(self.regs, off)
    }

    pub fn width(&self) -> usize {
        match self.reg(0) & 7 {
            0 | 4 => 320,
            1 | 5 => 352,
            2 | 6 => 640,
            _ => 704,
        }
    }

    /// Lines of the picture: 224, 240 or 256, twice that in double-density interlace (both fields).
    pub fn height(&self) -> usize {
        let h = match (self.reg(0) >> 4) & 3 {
            0 => 224,
            1 => 240,
            _ => 256,
        };
        if self.double_density() { h * 2 } else { h }
    }

    fn double_density(&self) -> bool {
        (self.reg(0) >> 6) & 3 == 3
    }

    /// 640 or 704 dots: the sprite layer (a 16-bit framebuffer) covers two dots a pixel.
    fn hires(&self) -> bool {
        self.reg(0) & 2 != 0
    }

    /// A CRAM colour (palette index) as 8-bit RGB, and its MSB (colour calculation control).
    fn cram_color(&self, index: usize) -> ([u8; 3], bool) {
        let mode = (self.reg(0x0E) >> 12) & 3;
        if mode == 2 {
            let a = (index & 0x3FF) * 4;
            let v = u32::from_be_bytes([self.cram[a], self.cram[a + 1], self.cram[a + 2], self.cram[a + 3]]);
            ([(v & 0xFF) as u8, (v >> 8) as u8, (v >> 16) as u8], v & 0x8000_0000 != 0)
        } else {
            let mask = if mode == 0 { 0x3FF } else { 0x7FF };
            let c = rd16(self.cram, (index & mask) * 2);
            (rgb555(c), c & 0x8000 != 0)
        }
    }

    /// Every CRAM colour, as [`Self::cram_color`] gives it for indices 0-2047.
    fn palette(&self) -> Vec<([u8; 3], bool)> {
        (0..2048).map(|i| self.cram_color(i)).collect()
    }

    /// Renders the frame: `width() * height()` RGB pixels.
    pub fn render(&self) -> Vec<u8> {
        let (w, h) = (self.width(), self.height());
        let mut out = vec![0u8; w * h * 3];
        // the layers in the order ties are broken: the sprites, RBG0, NBG0-3
        let mut layers: [Vec<Option<Px>>; 6] = std::array::from_fn(|_| vec![None; w]);
        let pal = self.palette();
        let debug = debug_pixel();
        for line in 0..h {
            let sprite_line = if self.double_density() { line / 2 } else { line };
            let [sprite, rbg0, nbg @ ..] = &mut layers;
            self.sprite_line(sprite_line, sprite, &pal);
            self.rbg0_line(line, rbg0, &pal);
            for (n, l) in nbg.iter_mut().enumerate() {
                self.nbg_line(n, line, l, &pal);
            }
            let back = self.back_color(line);
            for x in 0..w {
                // the top two layers by priority; ties go to the earlier layer
                let mut top: Option<(usize, Px)> = None;
                let mut second: Option<(usize, Px)> = None;
                for (i, l) in layers.iter().enumerate() {
                    let Some(p) = l[x] else { continue };
                    if p.prio == 0 {
                        continue;
                    }
                    if top.is_none_or(|(_, t)| p.prio > t.prio) {
                        second = top;
                        top = Some((i, p));
                    } else if second.is_none_or(|(_, s)| p.prio > s.prio) {
                        second = Some((i, p));
                    }
                }
                if let Some((dx, dy)) = debug
                    && dx == x
                    && dy == line
                {
                    eprintln!("pixel {x},{line}: top {top:?} second {second:?} back {back:?} ccctl {:04x}", self.reg(0xEC));
                }
                // colour calculation with the layer below (or the back screen), then the top
                // layer's colour offset
                let rgb = match top {
                    None => self.offset(back, self.back_offset()),
                    Some((_, t)) => {
                        let mut c = t.rgb;
                        if t.cc {
                            let under = second.map_or(back, |(_, s)| s.rgb);
                            c = if self.reg(0xEC) & 0x0100 != 0 {
                                [0, 1, 2].map(|k| (c[k] as u16 + under[k] as u16).min(0xFF) as u8)
                            } else {
                                // ratio r: the top layer weighs (31 - r) / 32, the one below (r + 1) / 32
                                let r = t.ratio as u16;
                                [0, 1, 2].map(|k| ((c[k] as u16 * (31 - r) + under[k] as u16 * (r + 1)) / 32) as u8)
                            };
                        }
                        self.offset(c, t.offset)
                    }
                };
                let o = (line * w + x) * 3;
                out[o..o + 3].copy_from_slice(&rgb);
            }
        }
        out
    }

    /// Colour offset `which` (0: A, 1: B) added to a colour.
    fn offset(&self, c: [u8; 3], which: Option<usize>) -> [u8; 3] {
        let Some(sel) = which else { return c };
        let base = if sel == 0 { 0x114 } else { 0x11A };
        let mut out = c;
        for (k, o) in out.iter_mut().enumerate() {
            let d = ((self.reg(base + 2 * k) as i32) << 23) >> 23;
            *o = (*o as i32 + d).clamp(0, 255) as u8;
        }
        out
    }

    fn offset_for(&self, bit: u16) -> Option<usize> {
        if self.reg(0x110) & bit != 0 { Some((self.reg(0x112) & bit != 0) as usize) } else { None }
    }

    fn back_offset(&self) -> Option<usize> {
        self.offset_for(1 << 5)
    }

    /// The back screen's colour for a line (BKTA: one colour, or one a line).
    fn back_color(&self, line: usize) -> [u8; 3] {
        let bkta = ((self.reg(0xAC) as u32) << 16) | self.reg(0xAE) as u32;
        let per_line = bkta & 0x8000_0000 != 0;
        let mut a = ((bkta & 0x7FFFF) * 2) as usize;
        if per_line {
            a += line * 2;
        }
        rgb555(rd16(self.vram, a & 0x7FFFE))
    }

    // ---- NBG0-3 --------------------------------------------------------------------------------

    fn nbg_line(&self, n: usize, line: usize, out: &mut [Option<Px>], pal: &[([u8; 3], bool)]) {
        out.fill(None);
        let bgon = self.reg(0x20);
        if bgon & (1 << n) == 0 || self.reg(0) & 0x8000 == 0 {
            return;
        }
        // NBG0 and NBG1 can be bitmaps (not covered yet)
        let (chctl, sh) = if n < 2 { (self.reg(0x28), 8 * n) } else { (self.reg(0x2A), 4 * (n - 2)) };
        let chsz = (chctl >> sh) & 1;
        if n < 2 && (chctl >> sh) & 2 != 0 {
            return;
        }
        let colors = if n == 0 {
            (chctl >> 4) & 7
        } else if n == 1 {
            (chctl >> 12) & 3
        } else {
            (chctl >> (sh + 1)) & 1
        };
        if colors > 1 {
            return; // 2048 colours and RGB cells: not covered yet
        }
        let pncn = self.reg(0x30 + 2 * n);
        let plsz = (self.reg(0x3A) >> (2 * n)) & 3;
        let mpof = ((self.reg(0x3C) >> (4 * n)) & 7) as u32;
        let planes = [
            (self.reg(0x40 + 4 * n) & 0x3F) as u32,
            ((self.reg(0x40 + 4 * n) >> 8) & 0x3F) as u32,
            (self.reg(0x42 + 4 * n) & 0x3F) as u32,
            ((self.reg(0x42 + 4 * n) >> 8) & 0x3F) as u32,
        ];
        let transparent_shown = bgon & (0x100 << n) != 0;
        let prio = if n < 2 { (self.reg(0xF8) >> (8 * n)) & 7 } else { (self.reg(0xFA) >> (8 * (n - 2))) & 7 } as u8;
        let cc_on = self.reg(0xEC) & (1 << n) != 0;
        let ratio = (self.reg(0x108 + 2 * (n / 2)) >> (8 * (n % 2))) as u8 & 0x1F;
        let offset = self.offset_for(1 << n);
        let craof = ((self.reg(0xE4) >> (4 * n)) & 7) as usize * 0x100;
        let sfprmd = (self.reg(0xEA) >> (2 * n)) & 3;
        let sfccmd = (self.reg(0xEE) >> (2 * n)) & 3;
        // scroll and zoom (NBG0/1 with fractions, 16.16; NBG2/3 integers)
        let (sx, sy, dx, dy) = if n < 2 {
            let b = 0x70 + 0x10 * n;
            let fx = |i: usize, d: usize| (((self.reg(i) as u32 & 0x7FF) << 16) | (self.reg(d) as u32 & 0xFF00)) as i64;
            let zx = (((self.reg(b + 8) as u32 & 7) << 16) | (self.reg(b + 10) as u32 & 0xFF00)) as i64;
            let zy = (((self.reg(b + 12) as u32 & 7) << 16) | (self.reg(b + 14) as u32 & 0xFF00)) as i64;
            (fx(b, b + 2), fx(b + 4, b + 6), if zx == 0 { 0x10000 } else { zx }, if zy == 0 { 0x10000 } else { zy })
        } else {
            let b = 0x90 + 4 * (n - 2);
            (((self.reg(b) & 0x7FF) as i64) << 16, ((self.reg(b + 2) & 0x7FF) as i64) << 16, 0x10000, 0x10000)
        };
        let two_word = pncn & 0x8000 == 0;
        // sizes as powers of two: characters 8 or 16 dots, pages 64 or 32 characters across,
        // planes 1 or 2 pages each way
        let cell_sh = 3 + chsz as usize;
        let cell = 1 << cell_sh;
        let page_cells_sh = 6 - chsz as usize;
        let pn_bytes = if two_word { 4 } else { 2 };
        let page_bytes = (1 << (2 * page_cells_sh)) * pn_bytes;
        let plane_w_sh = if plsz & 1 != 0 { 10 } else { 9 };
        let plane_h_sh = if plsz & 2 != 0 { 10 } else { 9 };
        let y = ((sy + line as i64 * dy) >> 16) as usize;
        let mut last: Option<(usize, PatternName)> = None;
        for (x, px) in out.iter_mut().enumerate() {
            let xx = ((sx + x as i64 * dx) >> 16) as usize;
            let (mx, my) = (xx & ((2 << plane_w_sh) - 1), y & ((2 << plane_h_sh) - 1));
            let plane = ((my >> plane_h_sh) << 1) | (mx >> plane_w_sh);
            let (px_in, py_in) = (mx & ((1 << plane_w_sh) - 1), my & ((1 << plane_h_sh) - 1));
            let page = ((py_in >> 9) << 1) | (px_in >> 9);
            let (pxp, pyp) = (px_in & 511, py_in & 511);
            let map = ((mpof << 6) | planes[plane]) & !(match plsz { 1 => 1, 3 => 3, _ => 0 });
            let plane_addr = (map as usize * page_bytes) & 0x7_FFFF;
            let entry = ((pyp >> cell_sh) << page_cells_sh) + (pxp >> cell_sh);
            let pa = (plane_addr + page * page_bytes + entry * pn_bytes) & 0x7_FFFF;
            let pn = match last {
                Some((a, pn)) if a == pa => pn,
                _ => {
                    let pn = if two_word {
                        PatternName::two_word(rd16(self.vram, pa), rd16(self.vram, (pa + 2) & 0x7_FFFF))
                    } else {
                        PatternName::one_word(rd16(self.vram, pa), pncn, colors, chsz)
                    };
                    last = Some((pa, pn));
                    pn
                }
            };
            // the dot within the character
            let (mut cx, mut cy) = (pxp & (cell - 1), pyp & (cell - 1));
            if pn.hflip {
                cx = cell - 1 - cx;
            }
            if pn.vflip {
                cy = cell - 1 - cy;
            }
            let cell_index = ((cy >> 3) << 1) | (cx >> 3); // 2x2 characters: cells left to right, top to bottom
            let (ix, iy) = (cx & 7, cy & 7);
            let cell_bytes = if colors == 0 { 32 } else { 64 };
            let base = pn.char_number as usize * 32 + cell_index * cell_bytes;
            let dot = if colors == 0 {
                let b = self.vram[(base + iy * 4 + ix / 2) & 0x7_FFFF];
                if ix & 1 == 0 { b >> 4 } else { b & 0xF }
            } else {
                self.vram[(base + iy * 8 + ix) & 0x7_FFFF]
            } as usize;
            if dot == 0 && !transparent_shown {
                continue;
            }
            let index = if colors == 0 { pn.palette as usize * 16 + dot } else { (pn.palette as usize & 0x70) * 16 + dot };
            let (rgb, msb) = pal[(index + craof) & 0x7FF];
            let mut p = prio;
            if sfprmd == 1 {
                p = (p & 6) | pn.spr as u8;
            }
            let cc = cc_on
                && match sfccmd {
                    1 => pn.scc,
                    3 => msb,
                    _ => true,
                };
            *px = Some(Px { rgb, prio: p, cc, ratio, offset });
        }
    }

    // ---- RBG0 ----------------------------------------------------------------------------------------

    fn rbg0_line(&self, line: usize, out: &mut [Option<Px>], pal: &[([u8; 3], bool)]) {
        out.fill(None);
        let bgon = self.reg(0x20);
        if bgon & 0x10 == 0 || self.reg(0) & 0x8000 == 0 {
            return;
        }
        let chctl = self.reg(0x2A);
        if chctl & 0x200 != 0 {
            return; // bitmaps: not covered yet
        }
        let colors = (chctl >> 12) & 7;
        if colors > 1 {
            return;
        }
        let chsz = (chctl >> 8) & 1;
        // rotation parameter A's table
        let rpta = ((((self.reg(0xBC) as usize) & 7) << 16) | (self.reg(0xBE) as usize & 0xFFFE)) * 2;
        let t = |off: usize| -> i64 {
            let a = (rpta + off) & 0x7_FFFC;
            i32::from_be_bytes([self.vram[a], self.vram[a + 1], self.vram[a + 2], self.vram[a + 3]]) as i64
        };
        let t16 = |off: usize| -> i64 { rd16(self.vram, (rpta + off) & 0x7_FFFE) as i16 as i64 };
        // fixed point: positions and matrix entries with 10 fraction bits (the table's low 6 bits
        // are below the precision), k with 16
        let fx = |v: i64, int_bits: u32| -> i64 { (v << (32 - (int_bits + 16))) >> (32 - (int_bits + 16)) >> 6 };
        let (xst, yst, zst) = (fx(t(0x00), 13), fx(t(0x04), 13), fx(t(0x08), 13));
        let (dxst, dyst) = (fx(t(0x0C), 3), fx(t(0x10), 3));
        let (dx, dy) = (fx(t(0x14), 3), fx(t(0x18), 3));
        let m: Vec<i64> = (0..6).map(|i| fx(t(0x1C + 4 * i), 4)).collect();
        let (px, py, pz) = (t16(0x34) << 10, t16(0x36) << 10, t16(0x38) << 10);
        let (cx, cy, cz) = (t16(0x3C) << 10, t16(0x3E) << 10, t16(0x40) << 10);
        let (mx, my) = (fx(t(0x44), 14), fx(t(0x48), 14));
        let (kx, ky) = ((t(0x4C) << 40) >> 40, (t(0x50) << 40) >> 40);
        let (kast, dkast, dkax) = (t(0x54) >> 6, fx(t(0x58), 10), fx(t(0x5C), 10));
        let mul = |a: i64, b: i64| (a * b) >> 10;
        let y = line as i64;
        let (xs, ys) = (xst + dxst * y, yst + dyst * y);
        let xsp = mul(m[0], xs - px) + mul(m[1], ys - py) + mul(m[2], zst - pz);
        let ysp = mul(m[3], xs - px) + mul(m[4], ys - py) + mul(m[5], zst - pz);
        let xp = mul(m[0], px - cx) + mul(m[1], py - cy) + mul(m[2], pz - cz) + cx + mx;
        let yp = mul(m[3], px - cx) + mul(m[4], py - cy) + mul(m[5], pz - cz) + cy + my;
        let ddx = mul(m[0], dx) + mul(m[1], dy);
        let ddy = mul(m[3], dx) + mul(m[4], dy);
        // the coefficient table
        let ktctl = self.reg(0xB4);
        let kt_on = ktctl & 1 != 0;
        let kt_one_word = ktctl & 2 != 0;
        let kt_mode = (ktctl >> 2) & 3;
        let ktaof = ((self.reg(0xB6) & 7) as usize) << 16;
        let coefficient = |ka: i64| -> Option<i64> {
            let i = ktaof | ((ka >> 10) as usize & 0xFFFF);
            if kt_one_word {
                let v = rd16(self.vram, (i * 2) & 0x7_FFFE);
                if v & 0x8000 != 0 { None } else { Some((((v as i64) << 49) >> 49) << 6) }
            } else {
                let a = (i * 4) & 0x7_FFFC;
                let v = u32::from_be_bytes([self.vram[a], self.vram[a + 1], self.vram[a + 2], self.vram[a + 3]]);
                if v & 0x8000_0000 != 0 { None } else { Some(((v as i64) << 40) >> 40) }
            }
        };
        let ka_line = kast + dkast * y;
        // the map: 16 planes (A-P) of RAPLSZ pages
        let pncn = self.reg(0x38);
        let plsz = (self.reg(0x3A) >> 8) & 3;
        let over = (self.reg(0x3A) >> 10) & 3;
        let mpof = (self.reg(0x3E) & 7) as u32;
        let planes: Vec<u32> = (0..16).map(|i| ((self.reg(0x50 + 2 * (i / 2)) >> (8 * (i % 2))) & 0x3F) as u32).collect();
        let transparent_shown = bgon & 0x1000 != 0;
        let prio = (self.reg(0xFC) & 7) as u8;
        let cc_on = self.reg(0xEC) & 0x10 != 0;
        let ratio = (self.reg(0x10C) & 0x1F) as u8;
        let offset = self.offset_for(1 << 4);
        let craof = (self.reg(0xE6) & 7) as usize * 0x100;
        let sfprmd = (self.reg(0xEA) >> 8) & 3;
        let sfccmd = (self.reg(0xEE) >> 8) & 3;
        let two_word = pncn & 0x8000 == 0;
        let cell = if chsz == 0 { 8 } else { 16 };
        let page_cells = if chsz == 0 { 64 } else { 32 };
        let pn_bytes = if two_word { 4 } else { 2 };
        let page_bytes = page_cells * page_cells * pn_bytes;
        let plane_w = if plsz & 1 != 0 { 1024 } else { 512 };
        let plane_h = if plsz & 2 != 0 { 1024 } else { 512 };
        for (x, out_px) in out.iter_mut().enumerate() {
            let (mut kxv, mut kyv, mut xpv) = (kx, ky, xp);
            if kt_on {
                let Some(k) = coefficient(ka_line + dkax * x as i64) else { continue };
                match kt_mode {
                    0 => {
                        kxv = k;
                        kyv = k;
                    }
                    1 => kxv = k,
                    2 => kyv = k,
                    _ => xpv = k >> 6,
                }
            }
            let xx = ((kxv * (xsp + ddx * x as i64)) >> 16) + xpv;
            let yy = ((kyv * (ysp + ddy * x as i64)) >> 16) + yp;
            let (ix, iy) = (xx >> 10, yy >> 10);
            let map_w = plane_w as i64 * 4;
            let map_h = plane_h as i64 * 4;
            if over == 2 && (ix < 0 || iy < 0 || ix >= map_w || iy >= map_h) || over == 3 && !(0..512).contains(&ix) | !(0..512).contains(&iy) {
                continue;
            }
            let (mx_, my_) = (ix.rem_euclid(map_w) as usize, iy.rem_euclid(map_h) as usize);
            let plane = (my_ / plane_h) * 4 + mx_ / plane_w;
            let (pxi, pyi) = (mx_ % plane_w, my_ % plane_h);
            let page = (pyi / 512) * 2 + pxi / 512;
            let (pxp, pyp) = (pxi % 512, pyi % 512);
            let map = ((mpof << 6) | planes[plane]) & !(match plsz { 1 => 1, 3 => 3, _ => 0 });
            let plane_addr = (map as usize * page_bytes) & 0x7_FFFF;
            let entry = (pyp / cell) * page_cells + pxp / cell;
            let pa = (plane_addr + page * page_bytes + entry * pn_bytes) & 0x7_FFFF;
            let pn = if two_word {
                PatternName::two_word(rd16(self.vram, pa), rd16(self.vram, (pa + 2) & 0x7_FFFF))
            } else {
                PatternName::one_word(rd16(self.vram, pa), pncn, colors, chsz)
            };
            let (mut cx_, mut cy_) = (pxp % cell, pyp % cell);
            if pn.hflip {
                cx_ = cell - 1 - cx_;
            }
            if pn.vflip {
                cy_ = cell - 1 - cy_;
            }
            let cell_index = (cy_ / 8) * 2 + cx_ / 8;
            let (dxi, dyi) = (cx_ % 8, cy_ % 8);
            let cell_bytes = if colors == 0 { 32 } else { 64 };
            let base = pn.char_number as usize * 32 + cell_index * cell_bytes;
            let dot = if colors == 0 {
                let b = self.vram[(base + dyi * 4 + dxi / 2) & 0x7_FFFF];
                if dxi & 1 == 0 { b >> 4 } else { b & 0xF }
            } else {
                self.vram[(base + dyi * 8 + dxi) & 0x7_FFFF]
            } as usize;
            if dot == 0 && !transparent_shown {
                continue;
            }
            let index = if colors == 0 { pn.palette as usize * 16 + dot } else { (pn.palette as usize & 0x70) * 16 + dot };
            let (rgb, msb) = pal[(index + craof) & 0x7FF];
            let mut p = prio;
            if sfprmd == 1 {
                p = (p & 6) | pn.spr as u8;
            }
            let cc = cc_on
                && match sfccmd {
                    1 => pn.scc,
                    3 => msb,
                    _ => true,
                };
            *out_px = Some(Px { rgb, prio: p, cc, ratio, offset });
        }
    }

    // ---- the sprite layer ------------------------------------------------------------------------

    fn sprite_line(&self, line: usize, out: &mut [Option<Px>], pal: &[([u8; 3], bool)]) {
        out.fill(None);
        let spctl = self.reg(0xE0);
        let sptype = spctl & 0xF;
        let mixed = spctl & 0x20 != 0;
        if sptype >= 8 || self.tvmr & 1 != 0 {
            return; // 8-bit framebuffers: not covered yet
        }
        let cc_on = self.reg(0xEC) & 0x40 != 0;
        let cccs = (spctl >> 12) & 3;
        let ccn = ((spctl >> 8) & 7) as u8;
        let offset = self.offset_for(1 << 6);
        let craof = ((self.reg(0xE6) >> 4) & 7) as usize * 0x100;
        let prio_of = |r: usize| ((self.reg(0xF0 + 2 * (r / 2)) >> (8 * (r % 2))) & 7) as u8;
        let ratio_of = |r: usize| ((self.reg(0x100 + 2 * (r / 2)) >> (8 * (r % 2))) & 0x1F) as u8;
        // (priority bits, their shift; colour-calculation bits, their shift; colour bits)
        let (pr_mask, pr_sh, cc_mask, cc_sh, dc_mask): (u16, u16, u16, u16, u16) = match sptype {
            0 => (3, 14, 7, 11, 0x7FF),
            1 => (7, 13, 3, 11, 0x7FF),
            2 => (1, 14, 7, 11, 0x7FF),
            3 => (3, 13, 3, 11, 0x7FF),
            4 => (3, 13, 7, 10, 0x3FF),
            5 => (7, 12, 1, 11, 0x7FF),
            6 => (7, 12, 3, 10, 0x3FF),
            _ => (7, 12, 7, 9, 0x1FF),
        };
        let hires = self.hires();
        for (x, px) in out.iter_mut().enumerate() {
            let fx = if hires { x / 2 } else { x };
            let v = rd16(self.fb, (line * 512 + fx) * 2);
            if v == 0 {
                continue;
            }
            if mixed && v & 0x8000 != 0 {
                // an RGB pixel: sprite register 0's priority and ratio
                let p = prio_of(0);
                let cc = cc_on && sprite_cc(cccs, p, ccn, true);
                *px = Some(Px { rgb: rgb555(v), prio: p, cc, ratio: ratio_of(0), offset });
                continue;
            }
            let pr = ((v >> pr_sh) & pr_mask) as usize;
            let ccr = ((v >> cc_sh) & cc_mask) as usize;
            let dc = (v & dc_mask) as usize;
            let (rgb, msb) = pal[(dc + craof) & 0x7FF];
            let p = prio_of(pr);
            let cc = cc_on && sprite_cc(cccs, p, ccn, msb);
            *px = Some(Px { rgb, prio: p, cc, ratio: ratio_of(ccr), offset });
        }
    }
}

/// STV_VDP2_DEBUG=x,y: print the layers at one pixel (development).
fn debug_pixel() -> Option<(usize, usize)> {
    let v = std::env::var("STV_VDP2_DEBUG").ok()?;
    let (x, y) = v.split_once(',')?;
    Some((x.parse().ok()?, y.parse().ok()?))
}

/// The sprite colour-calculation condition (SPCTL SPCCCS) for a pixel's priority.
fn sprite_cc(cccs: u16, prio: u8, ccn: u8, msb: bool) -> bool {
    match cccs {
        0 => prio <= ccn,
        1 => prio == ccn,
        2 => prio >= ccn,
        _ => msb,
    }
}

fn rgb555(c: u16) -> [u8; 3] {
    [((c & 0x1F) << 3) as u8, (((c >> 5) & 0x1F) << 3) as u8, (((c >> 10) & 0x1F) << 3) as u8]
}

/// A decoded pattern name.
#[derive(Clone, Copy)]
struct PatternName {
    char_number: u32,
    palette: u32,
    hflip: bool,
    vflip: bool,
    spr: bool,
    scc: bool,
}

impl PatternName {
    fn two_word(w0: u16, w1: u16) -> PatternName {
        PatternName {
            char_number: (w1 & 0x7FFF) as u32,
            palette: (w0 & 0x7F) as u32,
            vflip: w0 & 0x8000 != 0,
            hflip: w0 & 0x4000 != 0,
            spr: w0 & 0x2000 != 0,
            scc: w0 & 0x1000 != 0,
        }
    }

    /// One-word pattern names: the rest of the fields come from PNCNx (supplement bits).
    fn one_word(w: u16, pncn: u16, colors: u16, chsz: u16) -> PatternName {
        let scn = (pncn & 0x1F) as u32;
        let splt = ((pncn >> 5) & 7) as u32;
        let cnsm = pncn & 0x4000 != 0;
        let (palette, data) = if colors == 0 {
            ((splt << 4) | ((w >> 12) & 0xF) as u32, w & 0x0FFF)
        } else {
            ((((w >> 12) & 7) as u32) << 4, w & 0x0FFF)
        };
        let (hflip, vflip, num) = if cnsm {
            (false, false, data as u32)
        } else {
            (data & 0x400 != 0, data & 0x800 != 0, (data & 0x3FF) as u32)
        };
        let char_number = match (cnsm, chsz) {
            (false, 0) => (scn << 10) | num,
            (false, _) => ((scn & 0x1C) << 10) | (num << 2) | (scn & 3),
            (true, 0) => ((scn & 0x1C) << 10) | num,
            (true, _) => ((scn & 0x10) << 10) | (num << 2) | (scn & 3),
        };
        PatternName {
            char_number,
            palette,
            hflip,
            vflip,
            spr: pncn & 0x200 != 0,
            scc: pncn & 0x100 != 0,
        }
    }
}
