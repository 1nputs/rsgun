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

/// One layer's pixel, packed so that of two pixels the one shown over the other is the greater:
/// its priority, then its layer (earlier layers win ties), above its colour-calculation flag and
/// ratio and its colour (8 bits a channel). Priority 0 shows nothing, so it is no pixel: 0. Colour
/// offset is the layer's.
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
struct Px(u64);

impl Px {
    fn new(layer: usize, rgb: [u8; 3], prio: u8, ratio: u8, cc: bool) -> Px {
        if prio == 0 {
            return Px(0);
        }
        let rgb = u32::from_le_bytes([rgb[0], rgb[1], rgb[2], 0]) as u64;
        Px((prio as u64) << 36 | ((5 - layer) as u64) << 32 | (cc as u64) << 29 | (ratio as u64) << 24 | rgb)
    }

    fn prio(self) -> u8 {
        (self.0 >> 36) as u8
    }

    fn layer(self) -> usize {
        5 - (self.0 >> 32 & 7) as usize
    }

    fn cc(self) -> bool {
        self.0 >> 29 & 1 != 0
    }

    fn ratio(self) -> u8 {
        (self.0 >> 24 & 0x1F) as u8
    }

    fn rgb(self) -> [u8; 3] {
        let [r, g, b, _] = (self.0 as u32).to_le_bytes();
        [r, g, b]
    }
}

impl std::fmt::Debug for Px {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if self.prio() == 0 {
            return write!(f, "none");
        }
        write!(f, "layer {} rgb {:?} prio {} ratio {} cc {}", self.layer(), self.rgb(), self.prio(), self.ratio(), self.cc())
    }
}

/// The layers, in the order priority ties are broken: the sprites, RBG0, NBG0-3.
const SPRITES: usize = 0;
const RBG0: usize = 1;
const NBG0: usize = 2;

/// The palette as the screens see it: each CRAM colour and its MSB.
type Palette = [([u8; 3], bool)];

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
        self.render_in(1)
    }

    /// [`Self::render`] in up to `threads` bands of lines at once (the same picture).
    pub fn render_in(&self, threads: usize) -> Vec<u8> {
        let (w, h) = (self.width(), self.height());
        let mut out = vec![0u8; w * h * 3];
        let settings = self.settings();
        let band = h.div_ceil(threads.clamp(1, h));
        if band == h {
            self.render_lines(&settings, 0, &mut out);
        } else {
            std::thread::scope(|scope| {
                for (i, rows) in out.chunks_mut(band * w * 3).enumerate() {
                    let settings = &settings;
                    scope.spawn(move || self.render_lines(settings, i * band, rows));
                }
            });
        }
        out
    }

    /// The settings that hold for the whole frame, and the layers they leave shown.
    fn settings(&self) -> Settings {
        let sprites = self.sprites();
        let rbg0 = self.rbg0();
        let nbgs: [Option<Nbg>; 4] = std::array::from_fn(|n| self.nbg(n));
        let shown = (0..6)
            .filter(|&i| match i {
                SPRITES => sprites.is_some(),
                RBG0 => rbg0.is_some(),
                _ => nbgs[i - NBG0].is_some(),
            })
            .collect();
        Settings {
            pal: self.palette(),
            sprites,
            rbg0,
            nbgs,
            shown,
            // colour offset as each layer adds it (0 when it has none), and the back screen's
            offsets: [1 << 6, 1 << 4, 1, 1 << 1, 1 << 2, 1 << 3, 1 << 5].map(|bit| match self.offset_for(bit) {
                None => [0; 3],
                Some(sel) => [0, 1, 2].map(|k| ((self.reg([0x114, 0x11A][sel] + 2 * k) as i32) << 23) >> 23),
            }),
            add: self.reg(0xEC) & 0x0100 != 0,
        }
    }

    /// Renders the lines from `first` into `rows` (whole lines of RGB pixels).
    fn render_lines(&self, f: &Settings, first: usize, rows: &mut [u8]) {
        let w = self.width();
        let pal = &f.pal;
        let mut layers: [Vec<Px>; 6] = std::array::from_fn(|_| vec![Px(0); w]);
        let (mut tops, mut seconds) = (vec![Px(0); w], vec![Px(0); w]);
        let debug = debug_pixel();
        let mut sprites_drawn = None;
        for (line, row) in (first..).zip(rows.chunks_exact_mut(w * 3)) {
            if let Some(s) = &f.sprites {
                // double-density interlace shows each framebuffer line twice
                let sprite_line = if self.double_density() { line / 2 } else { line };
                if sprites_drawn != Some(sprite_line) {
                    self.sprite_line(s, sprite_line, &mut layers[SPRITES], pal);
                    sprites_drawn = Some(sprite_line);
                }
            }
            if let Some(r) = &f.rbg0 {
                self.rbg0_line(r, line, &mut layers[RBG0], pal);
            }
            for (n, g) in f.nbgs.iter().enumerate() {
                if let Some(g) = g {
                    self.nbg_line(g, line, &mut layers[NBG0 + n], pal);
                }
            }
            // the top two layers by priority; ties go to the earlier layer
            tops.fill(Px(0));
            seconds.fill(Px(0));
            for &i in &f.shown {
                for ((top, second), &p) in tops.iter_mut().zip(seconds.iter_mut()).zip(&layers[i]) {
                    *second = (*second).max((*top).min(p));
                    *top = (*top).max(p);
                }
            }
            let back = self.back_color(line);
            if let Some((x, y)) = debug
                && y == line
            {
                eprintln!("pixel {x},{line}: top {:?} second {:?} back {back:?} ccctl {:04x}", tops[x], seconds[x], self.reg(0xEC));
            }
            // colour calculation with the layer below (or the back screen), then the top layer's
            // colour offset
            for ((rgb_out, &top), &second) in row.as_chunks_mut::<3>().0.iter_mut().zip(&tops).zip(&seconds) {
                let (mut c, layer) = if top == Px(0) { (back, 6) } else { (top.rgb(), top.layer()) };
                if top.cc() {
                    let under = if second == Px(0) { back } else { second.rgb() };
                    c = if f.add {
                        [0, 1, 2].map(|k| (c[k] as u16 + under[k] as u16).min(0xFF) as u8)
                    } else {
                        // ratio r: the top layer weighs (31 - r) / 32, the one below (r + 1) / 32
                        let r = top.ratio() as u16;
                        [0, 1, 2].map(|k| ((c[k] as u16 * (31 - r) + under[k] as u16 * (r + 1)) / 32) as u8)
                    };
                }
                let d = &f.offsets[layer];
                *rgb_out = [0, 1, 2].map(|k| (c[k] as i32 + d[k]).clamp(0, 255) as u8);
            }
        }
    }

    fn offset_for(&self, bit: u16) -> Option<usize> {
        if self.reg(0x110) & bit != 0 { Some((self.reg(0x112) & bit != 0) as usize) } else { None }
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

    /// A character's dot (a palette index within it) at row `row` (the address of its line, cell
    /// columns apart by `cell_bytes`), column `cx`.
    fn dot(&self, colors: u16, row: usize, cell_bytes: usize, cx: usize) -> usize {
        let a = row + (cx >> 3) * cell_bytes;
        let ix = cx & 7;
        (if colors == 0 {
            let b = self.vram[(a + ix / 2) & 0x7_FFFF];
            if ix & 1 == 0 { b >> 4 } else { b & 0xF }
        } else {
            self.vram[(a + ix) & 0x7_FFFF]
        }) as usize
    }

    // ---- NBG0-3 --------------------------------------------------------------------------------

    /// NBGn's settings for the frame: None when it shows nothing (off, or a kind not covered yet).
    fn nbg(&self, n: usize) -> Option<Nbg> {
        let bgon = self.reg(0x20);
        if bgon & (1 << n) == 0 || self.reg(0) & 0x8000 == 0 {
            return None;
        }
        // NBG0 and NBG1 can be bitmaps (not covered yet)
        let (chctl, sh) = if n < 2 { (self.reg(0x28), 8 * n) } else { (self.reg(0x2A), 4 * (n - 2)) };
        let chsz = (chctl >> sh) & 1;
        if n < 2 && (chctl >> sh) & 2 != 0 {
            return None;
        }
        let colors = if n == 0 {
            (chctl >> 4) & 7
        } else if n == 1 {
            (chctl >> 12) & 3
        } else {
            (chctl >> (sh + 1)) & 1
        };
        if colors > 1 {
            return None; // 2048 colours and RGB cells: not covered yet
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
        let page_cells_sh = 6 - chsz as usize;
        let pn_bytes = if two_word { 4 } else { 2 };
        let page_bytes = (1 << (2 * page_cells_sh)) * pn_bytes;
        let maps = planes.map(|p| ((((mpof << 6) | p) & !(match plsz { 1 => 1, 3 => 3, _ => 0 })) as usize * page_bytes) & 0x7_FFFF);
        Some(Nbg {
            screen: Screen {
                layer: NBG0 + n,
                colors,
                pncn,
                chsz,
                two_word,
                transparent_shown: bgon & (0x100 << n) != 0,
                prio: if n < 2 { (self.reg(0xF8) >> (8 * n)) & 7 } else { (self.reg(0xFA) >> (8 * (n - 2))) & 7 } as u8,
                cc_on: self.reg(0xEC) & (1 << n) != 0,
                ratio: (self.reg(0x108 + 2 * (n / 2)) >> (8 * (n % 2))) as u8 & 0x1F,
                craof: ((self.reg(0xE4) >> (4 * n)) & 7) as usize * 0x100,
                sfprmd: (self.reg(0xEA) >> (2 * n)) & 3,
                sfccmd: (self.reg(0xEE) >> (2 * n)) & 3,
            },
            maps,
            sx,
            sy,
            dx,
            dy,
            cell_sh,
            page_cells_sh,
            pn_bytes,
            page_bytes,
            plane_w_sh: if plsz & 1 != 0 { 10 } else { 9 },
            plane_h_sh: if plsz & 2 != 0 { 10 } else { 9 },
        })
    }

    fn nbg_line(&self, g: &Nbg, line: usize, out: &mut [Px], pal: &Palette) {
        let y = ((g.sy + line as i64 * g.dy) >> 16) as usize;
        let my = y & ((2 << g.plane_h_sh) - 1);
        let py_in = my & ((1 << g.plane_h_sh) - 1);
        let pyp = py_in & 511;
        // the line's row of planes, of pages, of pattern names, and its line in a character
        let plane_row = (my >> g.plane_h_sh) << 1;
        let page_row = (py_in >> 9) << 1;
        let entry_row = (pyp >> g.cell_sh) << g.page_cells_sh;
        let cell = 1 << g.cell_sh;
        let line_in_cell = pyp & (cell - 1);
        // the character under the last dot: its column in the map, and its line's dots
        let mut column = usize::MAX;
        let mut ch = CharacterLine::default();
        for (x, px) in out.iter_mut().enumerate() {
            let xx = ((g.sx + x as i64 * g.dx) >> 16) as usize;
            let mx = xx & ((2 << g.plane_w_sh) - 1);
            if mx >> g.cell_sh != column {
                column = mx >> g.cell_sh;
                let plane = plane_row | (mx >> g.plane_w_sh);
                let px_in = mx & ((1 << g.plane_w_sh) - 1);
                let page = page_row | (px_in >> 9);
                let entry = entry_row + ((px_in & 511) >> g.cell_sh);
                let pa = (g.maps[plane] + page * g.page_bytes + entry * g.pn_bytes) & 0x7_FFFF;
                ch = self.character_line(&g.screen, pa, line_in_cell);
            }
            *px = self.character_dot(&g.screen, &ch, mx & (cell - 1), pal);
        }
    }

    /// The pattern name at `pa` and its character's line `cy` (before flips).
    fn character_line(&self, s: &Screen, pa: usize, cy: usize) -> CharacterLine {
        let pn = if s.two_word {
            PatternName::two_word(rd16(self.vram, pa), rd16(self.vram, (pa + 2) & 0x7_FFFF))
        } else {
            PatternName::one_word(rd16(self.vram, pa), s.pncn, s.colors, s.chsz)
        };
        let cell = 8 << s.chsz;
        let cy = if pn.vflip { cell - 1 - cy } else { cy };
        // 2x2 characters: cells left to right, top to bottom
        let cell_bytes = if s.colors == 0 { 32 } else { 64 };
        let row = pn.char_number as usize * 32 + (cy >> 3) * 2 * cell_bytes + (cy & 7) * (cell_bytes / 8);
        let palette = if s.colors == 0 { pn.palette as usize * 16 } else { (pn.palette as usize & 0x70) * 16 };
        let prio = if s.sfprmd == 1 { (s.prio & 6) | pn.spr as u8 } else { s.prio };
        CharacterLine { row, hflip: pn.hflip, palette: palette + s.craof, prio, scc: pn.scc }
    }

    /// A character line's dot `cx` (before flips) as the screen's pixel.
    fn character_dot(&self, s: &Screen, ch: &CharacterLine, cx: usize, pal: &Palette) -> Px {
        let cell = 8 << s.chsz;
        let cx = if ch.hflip { cell - 1 - cx } else { cx };
        let dot = self.dot(s.colors, ch.row, if s.colors == 0 { 32 } else { 64 }, cx);
        if dot == 0 && !s.transparent_shown {
            return Px(0);
        }
        let (rgb, msb) = pal[(ch.palette + dot) & 0x7FF];
        let cc = s.cc_on
            && match s.sfccmd {
                1 => ch.scc,
                3 => msb,
                _ => true,
            };
        Px::new(s.layer, rgb, ch.prio, s.ratio, cc)
    }

    // ---- RBG0 ----------------------------------------------------------------------------------------

    /// RBG0's settings for the frame: None when it shows nothing (off, or a kind not covered yet).
    fn rbg0(&self) -> Option<Rbg0> {
        let bgon = self.reg(0x20);
        if bgon & 0x10 == 0 || self.reg(0) & 0x8000 == 0 {
            return None;
        }
        let chctl = self.reg(0x2A);
        if chctl & 0x200 != 0 {
            return None; // bitmaps: not covered yet
        }
        let colors = (chctl >> 12) & 7;
        if colors > 1 {
            return None;
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
        let ktctl = self.reg(0xB4);
        let pncn = self.reg(0x38);
        let plsz = (self.reg(0x3A) >> 8) & 3;
        let mpof = (self.reg(0x3E) & 7) as u32;
        let two_word = pncn & 0x8000 == 0;
        // sizes as powers of two: characters 8 or 16 dots, pages 64 or 32 characters across,
        // planes 1 or 2 pages each way
        let cell_sh = 3 + chsz as usize;
        let page_cells_sh = 6 - chsz as usize;
        let pn_bytes = if two_word { 4 } else { 2 };
        let page_bytes = (1 << (2 * page_cells_sh)) * pn_bytes;
        // the map: 16 planes (A-P) of RAPLSZ pages
        let maps: [usize; 16] = std::array::from_fn(|i| {
            let plane = ((self.reg(0x50 + 2 * (i / 2)) >> (8 * (i % 2))) & 0x3F) as u32;
            ((((mpof << 6) | plane) & !(match plsz { 1 => 1, 3 => 3, _ => 0 })) as usize * page_bytes) & 0x7_FFFF
        });
        Some(Rbg0 {
            screen: Screen {
                layer: RBG0,
                colors,
                pncn,
                chsz,
                two_word,
                transparent_shown: bgon & 0x1000 != 0,
                prio: (self.reg(0xFC) & 7) as u8,
                cc_on: self.reg(0xEC) & 0x10 != 0,
                ratio: (self.reg(0x10C) & 0x1F) as u8,
                craof: (self.reg(0xE6) & 7) as usize * 0x100,
                sfprmd: (self.reg(0xEA) >> 8) & 3,
                sfccmd: (self.reg(0xEE) >> 8) & 3,
            },
            start: [fx(t(0x00), 13), fx(t(0x04), 13), fx(t(0x08), 13)],
            line_step: [fx(t(0x0C), 3), fx(t(0x10), 3)],
            dot_step: [fx(t(0x14), 3), fx(t(0x18), 3)],
            m: std::array::from_fn(|i| fx(t(0x1C + 4 * i), 4)),
            view: [t16(0x34) << 10, t16(0x36) << 10, t16(0x38) << 10],
            centre: [t16(0x3C) << 10, t16(0x3E) << 10, t16(0x40) << 10],
            shift: [fx(t(0x44), 14), fx(t(0x48), 14)],
            k: [(t(0x4C) << 40) >> 40, (t(0x50) << 40) >> 40],
            ka: [t(0x54) >> 6, fx(t(0x58), 10), fx(t(0x5C), 10)],
            kt_on: ktctl & 1 != 0,
            kt_one_word: ktctl & 2 != 0,
            kt_mode: (ktctl >> 2) & 3,
            ktaof: ((self.reg(0xB6) & 7) as usize) << 16,
            over: (self.reg(0x3A) >> 10) & 3,
            maps,
            cell_sh,
            page_cells_sh,
            pn_bytes,
            page_bytes,
            plane_w_sh: if plsz & 1 != 0 { 10 } else { 9 },
            plane_h_sh: if plsz & 2 != 0 { 10 } else { 9 },
        })
    }

    /// The coefficient table's entry for KA (None: its MSB says the dot shows nothing).
    fn coefficient(&self, r: &Rbg0, ka: i64) -> Option<i64> {
        let i = r.ktaof | ((ka >> 10) as usize & 0xFFFF);
        if r.kt_one_word {
            let v = rd16(self.vram, (i * 2) & 0x7_FFFE);
            if v & 0x8000 != 0 { None } else { Some((((v as i64) << 49) >> 49) << 6) }
        } else {
            let a = (i * 4) & 0x7_FFFC;
            let v = u32::from_be_bytes([self.vram[a], self.vram[a + 1], self.vram[a + 2], self.vram[a + 3]]);
            if v & 0x8000_0000 != 0 { None } else { Some(((v as i64) << 40) >> 40) }
        }
    }

    fn rbg0_line(&self, r: &Rbg0, line: usize, out: &mut [Px], pal: &Palette) {
        let mul = |a: i64, b: i64| (a * b) >> 10;
        let m = &r.m;
        let ([xst, yst, zst], [px, py, pz], [cx, cy, cz]) = (r.start, r.view, r.centre);
        let y = line as i64;
        let (xs, ys) = (xst + r.line_step[0] * y, yst + r.line_step[1] * y);
        let xsp = mul(m[0], xs - px) + mul(m[1], ys - py) + mul(m[2], zst - pz);
        let ysp = mul(m[3], xs - px) + mul(m[4], ys - py) + mul(m[5], zst - pz);
        let xp = mul(m[0], px - cx) + mul(m[1], py - cy) + mul(m[2], pz - cz) + cx + r.shift[0];
        let yp = mul(m[3], px - cx) + mul(m[4], py - cy) + mul(m[5], pz - cz) + cy + r.shift[1];
        let ddx = mul(m[0], r.dot_step[0]) + mul(m[1], r.dot_step[1]);
        let ddy = mul(m[3], r.dot_step[0]) + mul(m[4], r.dot_step[1]);
        let [kast, dkast, dkax] = r.ka;
        let ka_line = kast + dkast * y;
        let map_w = 4i64 << r.plane_w_sh;
        let map_h = 4i64 << r.plane_h_sh;
        let cell = 1 << r.cell_sh;
        // the character under the last dot: its place in the map, and its line's dots
        let mut place = None;
        let mut ch = CharacterLine::default();
        for (x, out_px) in out.iter_mut().enumerate() {
            let [mut kxv, mut kyv] = r.k;
            let mut xpv = xp;
            if r.kt_on {
                let Some(k) = self.coefficient(r, ka_line + dkax * x as i64) else {
                    *out_px = Px(0);
                    continue;
                };
                match r.kt_mode {
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
            if r.over == 2 && (ix < 0 || iy < 0 || ix >= map_w || iy >= map_h) || r.over == 3 && !(0..512).contains(&ix) | !(0..512).contains(&iy) {
                *out_px = Px(0);
                continue;
            }
            // the map is a power of two each way
            let (mx, my) = ((ix & (map_w - 1)) as usize, (iy & (map_h - 1)) as usize);
            let key = (mx >> r.cell_sh, my);
            if place != Some(key) {
                place = Some(key);
                let plane = ((my >> r.plane_h_sh) << 2) | (mx >> r.plane_w_sh);
                let (pxi, pyi) = (mx & ((1 << r.plane_w_sh) - 1), my & ((1 << r.plane_h_sh) - 1));
                let page = ((pyi >> 9) << 1) | (pxi >> 9);
                let (pxp, pyp) = (pxi & 511, pyi & 511);
                let entry = ((pyp >> r.cell_sh) << r.page_cells_sh) + (pxp >> r.cell_sh);
                let pa = (r.maps[plane] + page * r.page_bytes + entry * r.pn_bytes) & 0x7_FFFF;
                ch = self.character_line(&r.screen, pa, pyp & (cell - 1));
            }
            *out_px = self.character_dot(&r.screen, &ch, mx & (cell - 1), pal);
        }
    }

    // ---- the sprite layer ------------------------------------------------------------------------

    /// The sprite layer's settings for the frame: None when it shows nothing (8-bit framebuffers,
    /// not covered yet).
    fn sprites(&self) -> Option<Sprites> {
        let spctl = self.reg(0xE0);
        let sptype = spctl & 0xF;
        if sptype >= 8 || self.tvmr & 1 != 0 {
            return None;
        }
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
        Some(Sprites {
            mixed: spctl & 0x20 != 0,
            cc_on: self.reg(0xEC) & 0x40 != 0,
            cccs: (spctl >> 12) & 3,
            ccn: ((spctl >> 8) & 7) as u8,
            craof: ((self.reg(0xE6) >> 4) & 7) as usize * 0x100,
            prios: std::array::from_fn(|r| ((self.reg(0xF0 + 2 * (r / 2)) >> (8 * (r % 2))) & 7) as u8),
            ratios: std::array::from_fn(|r| ((self.reg(0x100 + 2 * (r / 2)) >> (8 * (r % 2))) & 0x1F) as u8),
            pr_mask,
            pr_sh,
            cc_mask,
            cc_sh,
            dc_mask,
            hires: self.hires(),
        })
    }

    fn sprite_line(&self, s: &Sprites, line: usize, out: &mut [Px], pal: &Palette) {
        for (x, px) in out.iter_mut().enumerate() {
            let fx = if s.hires { x / 2 } else { x };
            let v = rd16(self.fb, (line * 512 + fx) * 2);
            *px = if v == 0 {
                Px(0)
            } else if s.mixed && v & 0x8000 != 0 {
                // an RGB pixel: sprite register 0's priority and ratio
                let p = s.prios[0];
                let cc = s.cc_on && sprite_cc(s.cccs, p, s.ccn, true);
                Px::new(SPRITES, rgb555(v), p, s.ratios[0], cc)
            } else {
                let pr = ((v >> s.pr_sh) & s.pr_mask) as usize;
                let ccr = ((v >> s.cc_sh) & s.cc_mask) as usize;
                let dc = (v & s.dc_mask) as usize;
                let (rgb, msb) = pal[(dc + s.craof) & 0x7FF];
                let p = s.prios[pr];
                let cc = s.cc_on && sprite_cc(s.cccs, p, s.ccn, msb);
                Px::new(SPRITES, rgb, p, s.ratios[ccr], cc)
            };
        }
    }
}

/// What holds for a whole frame: the palette, each layer's settings (None: it shows nothing), the
/// layers shown, colour offset as each layer adds it (the back screen's last), and whether colour
/// calculation adds.
struct Settings {
    pal: Vec<([u8; 3], bool)>,
    sprites: Option<Sprites>,
    rbg0: Option<Rbg0>,
    nbgs: [Option<Nbg>; 4],
    shown: Vec<usize>,
    offsets: [[i32; 3]; 7],
    add: bool,
}

/// What a cell screen (NBG0-3, RBG0) shows of its characters, and how.
struct Screen {
    layer: usize,
    colors: u16,
    pncn: u16,
    chsz: u16,
    two_word: bool,
    transparent_shown: bool,
    prio: u8,
    cc_on: bool,
    ratio: u8,
    craof: usize,
    sfprmd: u16,
    sfccmd: u16,
}

/// NBGn's settings for a frame: its map (each plane's address), scroll and zoom (16.16), and sizes
/// as powers of two.
struct Nbg {
    screen: Screen,
    maps: [usize; 4],
    sx: i64,
    sy: i64,
    dx: i64,
    dy: i64,
    cell_sh: usize,
    page_cells_sh: usize,
    pn_bytes: usize,
    page_bytes: usize,
    plane_w_sh: usize,
    plane_h_sh: usize,
}

/// RBG0's settings for a frame: rotation parameter A's table (fixed point, 10 fraction bits) and
/// coefficient table, its map of 16 planes, and sizes as powers of two.
struct Rbg0 {
    screen: Screen,
    start: [i64; 3],
    line_step: [i64; 2],
    dot_step: [i64; 2],
    m: [i64; 6],
    view: [i64; 3],
    centre: [i64; 3],
    shift: [i64; 2],
    k: [i64; 2],
    ka: [i64; 3],
    kt_on: bool,
    kt_one_word: bool,
    kt_mode: u16,
    ktaof: usize,
    over: u16,
    maps: [usize; 16],
    cell_sh: usize,
    page_cells_sh: usize,
    pn_bytes: usize,
    page_bytes: usize,
    plane_w_sh: usize,
    plane_h_sh: usize,
}

/// One line of a character as a screen shows it: the address of its dots (flipped vertically
/// already), its palette (with the screen's CRAM offset), priority and special colour calculation.
#[derive(Default)]
struct CharacterLine {
    row: usize,
    hflip: bool,
    palette: usize,
    prio: u8,
    scc: bool,
}

/// The sprite layer's settings for a frame.
struct Sprites {
    mixed: bool,
    cc_on: bool,
    cccs: u16,
    ccn: u8,
    craof: usize,
    prios: [u8; 8],
    ratios: [u8; 8],
    pr_mask: u16,
    pr_sh: u16,
    cc_mask: u16,
    cc_sh: u16,
    dc_mask: u16,
    hires: bool,
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
