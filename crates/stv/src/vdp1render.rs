//! VDP1's drawing: a command list in VRAM drawn into a framebuffer (512 x 256, 16-bit pixels). From
//! the VDP1 user's manual; the rasterization's details measured on the oracle's VDP1 with crafted
//! command lists (tools/vdp1/probes.py: 15 probes, every pixel equal) and checked on the game's own
//! draws (stv-check vdp1).
//!
//! Untextured commands (polygons, lines) still go through the texture's colour mode: they are drawn
//! only if the last dot of VRAM in that mode would be (the oracle's VDP1, tools/vdp1/probes.py):
//! not transparent unless SPD, not an end code unless ECD.
//!
//! Quadrilaterals (sprites and polygons) are drawn as VDP1 draws them: the left edge A-D and the
//! right edge B-C are stepped together over the longer edge's length, and a line is drawn from the
//! one to the other at every step, the texture's row following the step and its column the line.
//! Lines step one pixel along their longer axis, the other axis rounding to nearest with halves
//! toward the start. A quad's line adds a pixel wherever both coordinates step at once, on the
//! left of its direction, so no gaps open between lines; a line command has none, and is drawn
//! with its longer axis increasing.

use crate::mem::{rd16, wr16};

pub struct Vdp1Draw<'a> {
    pub vram: &'a [u8],
    pub fb: &'a mut [u8],
    pub tvmr: u16,
    sys_clip: (i32, i32),
    user_clip: (i32, i32, i32, i32),
    local: (i32, i32),
}

/// A command's drawing parameters.
#[derive(Clone, Copy)]
struct Draw {
    pmod: u16,
    colr: u16,
    tex: Option<(usize, usize, usize)>, // address, width, height
    hflip: bool,
    vflip: bool,
    gouraud: Option<[[i32; 3]; 4]>,
}

/// `num / den` (den > 0) rounded to nearest, halves toward zero.
fn round_half_in(num: i32, den: i32) -> i32 {
    let (q, r) = (num.abs() / den, num.abs() % den);
    let q = if 2 * r > den { q + 1 } else { q };
    if num < 0 { -q } else { q }
}

/// The texel (of `texels`) drawn at step `i` of `steps` (the oracle's VDP1, tools/vdp1/probes.py):
/// shrinking, the one under the step's centre; stretching, a Bresenham walk from the first texel to
/// the last that rounds halves down.
fn texel_at(i: i32, texels: i32, steps: i32) -> usize {
    if steps <= 1 {
        return (texels / 2) as usize;
    }
    if steps >= texels {
        ((2 * i * (texels - 1) + steps - 2) / (2 * (steps - 1))) as usize
    } else {
        (((2 * i + 1) * texels) / (2 * steps)) as usize
    }
}

/// A gouraud channel at step `i` of `steps` from `a` to `b`: its values walked as texels are,
/// from the lower value up.
fn gouraud_at(i: i32, a: i32, b: i32, steps: i32) -> i32 {
    if a <= b { a + texel_at(i, b - a + 1, steps) as i32 } else { b + texel_at(steps - 1 - i, a - b + 1, steps) as i32 }
}

fn sext(v: u16, bits: u32) -> i32 {
    ((v as i32) << (32 - bits)) >> (32 - bits)
}

impl<'a> Vdp1Draw<'a> {
    pub fn new(vram: &'a [u8], fb: &'a mut [u8], tvmr: u16) -> Vdp1Draw<'a> {
        Vdp1Draw { vram, fb, tvmr, sys_clip: (0, 0), user_clip: (0, 0, 0, 0), local: (0, 0) }
    }

    fn w(&self, a: usize, i: usize) -> u16 {
        rd16(self.vram, (a + 2 * i) & 0x7_FFFE)
    }

    /// Runs the command list from the start of VRAM to its end command.
    pub fn run(&mut self) {
        let mut addr = 0usize;
        let mut ret: Option<usize> = None;
        for _ in 0..0x8000 {
            let ctrl = self.w(addr, 0);
            if ctrl & 0x8000 != 0 {
                break;
            }
            let jp = (ctrl >> 12) & 7;
            if jp < 4 {
                self.command(addr);
            }
            let link = (self.w(addr, 1) as usize & 0xFFFC) << 3;
            addr = match jp & 3 {
                0 => addr + 0x20,
                1 => link,
                2 => {
                    ret = Some(addr + 0x20);
                    link
                }
                _ => ret.take().unwrap_or(addr + 0x20),
            } & 0x7_FFE0;
        }
    }

    fn vertex(&self, a: usize, i: usize) -> (i32, i32) {
        (sext(self.w(a, 6 + 2 * i), 13) + self.local.0, sext(self.w(a, 7 + 2 * i), 13) + self.local.1)
    }

    fn command(&mut self, a: usize) {
        let ctrl = self.w(a, 0);
        let pmod = self.w(a, 2);
        let colr = self.w(a, 3);
        let srca = self.w(a, 4) as usize * 8;
        let size = self.w(a, 5);
        let (tw, th) = (((size >> 8) & 0x3F) as usize * 8, (size & 0xFF) as usize);
        let gouraud = if pmod & 4 != 0 {
            let g = self.w(a, 14) as usize * 8;
            Some([0, 1, 2, 3].map(|i| {
                let c = self.w(g, i);
                [(c & 0x1F) as i32, ((c >> 5) & 0x1F) as i32, ((c >> 10) & 0x1F) as i32]
            }))
        } else {
            None
        };
        let tex = Draw {
            pmod,
            colr,
            tex: Some((srca, tw, th)),
            hflip: ctrl & 0x10 != 0,
            vflip: ctrl & 0x20 != 0,
            gouraud,
        };
        match ctrl & 0xF {
            0 => {
                let (x, y) = self.vertex(a, 0);
                let (w, h) = (tw as i32, th as i32);
                self.quad([(x, y), (x + w - 1, y), (x + w - 1, y + h - 1), (x, y + h - 1)], tex);
            }
            1 => {
                let (ax, ay) = self.vertex(a, 0);
                let zp = (ctrl >> 8) & 0xF;
                let (x0, y0, x1, y1) = if zp == 0 {
                    let (cx, cy) = self.vertex(a, 2);
                    (ax, ay, cx, cy)
                } else {
                    let (w, h) = (sext(self.w(a, 8), 13), sext(self.w(a, 9), 13));
                    // (halving rounds down: an odd negative size centres a pixel over)
                    let x0 = match zp & 3 {
                        2 => ax - (w >> 1),
                        3 => ax - w,
                        _ => ax,
                    };
                    let y0 = match (zp >> 2) & 3 {
                        2 => ay - (h >> 1),
                        3 => ay - h,
                        _ => ay,
                    };
                    (x0, y0, x0 + w, y0 + h)
                };
                self.quad([(x0, y0), (x1, y0), (x1, y1), (x0, y1)], tex);
            }
            2 | 3 => {
                let v = [0, 1, 2, 3].map(|i| self.vertex(a, i));
                self.quad(v, tex);
            }
            4 if self.untextured_drawn(pmod) => {
                let v = [0, 1, 2, 3].map(|i| self.vertex(a, i));
                self.quad(v, Draw { tex: None, ..tex });
            }
            5 | 7 if self.untextured_drawn(pmod) => {
                let v = [0, 1, 2, 3].map(|i| self.vertex(a, i));
                for i in 0..4 {
                    self.line(v[i], v[(i + 1) % 4], None, Draw { tex: None, ..tex }, None, false);
                }
            }
            6 if self.untextured_drawn(pmod) => {
                let (p0, p1) = (self.vertex(a, 0), self.vertex(a, 1));
                self.line(p0, p1, None, Draw { tex: None, ..tex }, None, false);
            }
            8 | 0xB => {
                self.user_clip = (self.w(a, 6) as i32 & 0x3FF, self.w(a, 7) as i32 & 0x1FF, self.w(a, 10) as i32 & 0x3FF, self.w(a, 11) as i32 & 0x1FF);
            }
            9 => self.sys_clip = (self.w(a, 10) as i32 & 0x3FF, self.w(a, 11) as i32 & 0x1FF),
            0xA => self.local = (sext(self.w(a, 6), 13), sext(self.w(a, 7), 13)),
            _ => {}
        }
    }

    /// A quadrilateral A, B, C, D: lines from the left edge (A-D) to the right (B-C).
    fn quad(&mut self, v: [(i32, i32); 4], d: Draw) {
        let [a, b, c, dd] = v;
        let len = |p: (i32, i32), q: (i32, i32)| (q.0 - p.0).abs().max((q.1 - p.1).abs());
        let n = len(a, dd).max(len(b, c));
        let th = d.tex.map_or(1, |t| t.2) as i32;
        for i in 0..=n {
            // a point of an edge, and its step along the edge (of its length)
            let lerp = |p: (i32, i32), q: (i32, i32)| -> ((i32, i32), i32, i32) {
                let (dx, dy) = (q.0 - p.0, q.1 - p.1);
                let m = dx.abs().max(dy.abs());
                if n == 0 || m == 0 {
                    return (p, 0, m);
                }
                // the edge is walked as a line command is (its longer axis increasing, halves toward
                // that start), the step mapped onto it the same way
                let x_major = dx.abs() >= dy.abs();
                let reversed = (x_major && dx < 0) || (!x_major && dy < 0);
                let (p, dx, dy, i) = if reversed { (q, -dx, -dy, n - i) } else { (p, dx, dy, i) };
                let j = round_half_in(m * i, n);
                let pt = if x_major { (p.0 + j, p.1 + round_half_in(dy * j, m)) } else { (p.0 + round_half_in(dx * j, m), p.1 + j) };
                (pt, if reversed { m - j } else { j }, m)
            };
            let ((l, jl, ml), (r, jr, mr)) = (lerp(a, dd), lerp(b, c));
            // a flip walks the texture from the other end
            let row = texel_at(if d.vflip { n - i } else { i }, th, n + 1);
            // each edge's gouraud walked over the edge's own steps
            let g = d.gouraud.map(|g| {
                let mix = |p: [i32; 3], q: [i32; 3], j: i32, m: i32| [0, 1, 2].map(|k| gouraud_at(j, p[k], q[k], m + 1));
                (mix(g[0], g[3], jl, ml), mix(g[1], g[2], jr, mr))
            });
            self.line(l, r, Some(row), d, g, true);
        }
    }

    /// A line from `p0` to `p1`: texture row `row` across it (or the command's colour), the
    /// gouraud colours at its ends; `quad`: a quad's line (its own direction, gap pixels), else a
    /// line command's (longer axis increasing, none).
    fn line(&mut self, p0: (i32, i32), p1: (i32, i32), row: Option<usize>, d: Draw, g: Option<([i32; 3], [i32; 3])>, quad: bool) {
        let (dx0, dy0) = (p1.0 - p0.0, p1.1 - p0.1);
        let x_major = dx0.abs() >= dy0.abs();
        let (p0, p1) = if !quad && ((x_major && dx0 < 0) || (!x_major && dy0 < 0)) { (p1, p0) } else { (p0, p1) };
        let (dx, dy) = (p1.0 - p0.0, p1.1 - p0.1);
        let len = dx.abs().max(dy.abs());
        let (sx, sy) = (dx.signum(), dy.signum());
        let tw = d.tex.map_or(1, |t| t.1) as i32;
        let mut prev: Option<(i32, i32)> = None;
        // end codes (ECD clear): the texels the walk passes are counted, the second ends the line
        let counting = d.pmod & 0x80 == 0 && matches!((d.tex, row), (Some(_), Some(_)));
        let (mut walked, mut end_codes) = (None::<i32>, 0);
        for j in 0..=len {
            let (x, y) = if len == 0 {
                p0
            } else if x_major {
                (p0.0 + sx * j, p0.1 + round_half_in(dy * j, len))
            } else {
                (p0.0 + round_half_in(dx * j, len), p0.1 + sy * j)
            };
            let walk = if d.hflip { len - j } else { j };
            let col = if d.pmod & 0x1000 != 0 && tw > len + 1 {
                // high-speed shrink: the even texels (odd with TVMR's EOS) walked as a texture of
                // half the width
                2 * texel_at(walk, tw / 2, len + 1) + ((self.tvmr >> 4) & 1) as usize
            } else {
                texel_at(walk, tw, len + 1)
            };
            if counting
                && let (Some((src, w, _)), Some(r)) = (d.tex, row)
            {
                let step = if d.hflip { -1 } else { 1 };
                let mut u = walked.map_or(if d.hflip { tw - 1 } else { 0 }, |u| u + step);
                while (step > 0 && u <= col as i32) || (step < 0 && u >= col as i32) {
                    if self.end_code(src, w, u as usize, r, d.pmod) {
                        end_codes += 1;
                    }
                    u += step;
                }
                walked = Some(col as i32);
                if end_codes >= 2 {
                    return;
                }
            }
            let gc = g.map(|(a, b)| [0, 1, 2].map(|k| gouraud_at(j, a[k], b[k], len + 1)));
            if quad
                && let Some((px, py)) = prev
                && px != x
                && py != y
            {
                // a diagonal step: the pixel that closes the gap, on the left of the direction
                if (x - px) * (y - py) > 0 {
                    self.pixel(x, py, row, col, d, gc);
                } else {
                    self.pixel(px, y, row, col, d, gc);
                }
            }
            self.pixel(x, y, row, col, d, gc);
            prev = Some((x, y));
        }
    }

    fn pixel(&mut self, x: i32, y: i32, row: Option<usize>, col: usize, d: Draw, g: Option<[i32; 3]>) {
        if x < 0 || y < 0 || x > self.sys_clip.0 || y > self.sys_clip.1 || x >= 512 || y >= 256 {
            return;
        }
        if d.pmod & 0x400 != 0 {
            let (x0, y0, x1, y1) = self.user_clip;
            let inside = x >= x0 && x <= x1 && y >= y0 && y <= y1;
            if inside == (d.pmod & 0x200 != 0) {
                return;
            }
        }
        if d.pmod & 0x100 != 0 && (x ^ y) & 1 != 0 {
            return;
        }
        let a = ((y as usize) * 512 + x as usize) * 2;
        let mut v = match (d.tex, row) {
            (Some((src, tw, th)), Some(r)) => {
                let _ = th;
                let Some(v) = self.texel(src, tw, col, r, d) else { return };
                v
            }
            _ => d.colr,
        };
        if d.pmod & 0x8000 != 0 {
            // MSB on: the pixel already there, with its MSB set
            let old = rd16(self.fb, a);
            wr16(self.fb, a, old | 0x8000);
            return;
        }
        if let Some(gc) = g
            && v & 0x8000 != 0
        {
            let ch = |s: u16, k: usize| ((((v >> s) & 0x1F) as i32 + gc[k] - 16).clamp(0, 31)) as u16;
            v = 0x8000 | ch(0, 0) | (ch(5, 1) << 5) | (ch(10, 2) << 10);
        }
        let old = rd16(self.fb, a);
        v = match d.pmod & 3 {
            1 => {
                // shadow: the pixel there, halved, if it is RGB
                if old & 0x8000 == 0 {
                    return;
                }
                0x8000 | ((old >> 1) & 0x3DEF)
            }
            2 => 0x8000 | ((v >> 1) & 0x3DEF),
            3 => {
                if old & 0x8000 != 0 {
                    // each channel the exact half of the sum
                    let half = |sh: u16| (((v >> sh) & 0x1F) + ((old >> sh) & 0x1F)) >> 1;
                    0x8000 | half(0) | (half(5) << 5) | (half(10) << 10)
                } else {
                    v
                }
            }
            _ => v,
        };
        wr16(self.fb, a, v);
    }

    /// Whether an untextured command draws: VRAM's last dot in the command's colour mode, taken as a
    /// texel (transparent unless SPD, an end code unless ECD).
    fn untextured_drawn(&self, pmod: u16) -> bool {
        let (spd, ecd) = (pmod & 0x40 != 0, pmod & 0x80 != 0);
        let (end, transparent) = match (pmod >> 3) & 7 {
            0 | 1 => {
                let d = self.vram[0x7_FFFF] & 0xF;
                (d == 0xF, d == 0)
            }
            2..=4 => {
                let d = self.vram[0x7_FFFF];
                (d == 0xFF, d == 0)
            }
            5 => {
                let w = rd16(self.vram, 0x7_FFFE) & 0xC000;
                (w == 0x4000, w == 0)
            }
            _ => return true,
        };
        if end { ecd } else { !transparent || spd }
    }

    /// Whether a texel is an end code in the command's colour mode (all ones; for RGB, 0x4000-0x7FFF).
    fn end_code(&self, src: usize, tw: usize, u: usize, r: usize, pmod: u16) -> bool {
        match (pmod >> 3) & 7 {
            0 | 1 => {
                let b = self.vram[(src + r * tw / 2 + u / 2) & 0x7_FFFF];
                (if u & 1 == 0 { b >> 4 } else { b & 0xF }) == 0xF
            }
            2..=4 => self.vram[(src + r * tw + u) & 0x7_FFFF] == 0xFF,
            5 => rd16(self.vram, (src + (r * tw + u) * 2) & 0x7_FFFE) & 0xC000 == 0x4000,
            _ => false,
        }
    }

    /// A texel through the command's colour mode; None when it is transparent, or an end code
    /// (ECD clear).
    fn texel(&self, src: usize, tw: usize, u: usize, r: usize, d: Draw) -> Option<u16> {
        if d.pmod & 0x80 == 0 && self.end_code(src, tw, u, r, d.pmod) {
            return None;
        }
        let mode = (d.pmod >> 3) & 7;
        let transparent_ok = d.pmod & 0x40 == 0;
        match mode {
            0 | 1 => {
                let b = self.vram[(src + r * tw / 2 + u / 2) & 0x7_FFFF];
                let t = if u & 1 == 0 { b >> 4 } else { b & 0xF } as u16;
                if t == 0 && transparent_ok {
                    return None;
                }
                Some(if mode == 0 {
                    (d.colr & 0xFFF0) | t
                } else {
                    rd16(self.vram, ((d.colr as usize & 0xFFFC) * 8 + t as usize * 2) & 0x7_FFFE)
                })
            }
            2..=4 => {
                let t = self.vram[(src + r * tw + u) & 0x7_FFFF] as u16;
                if t == 0 && transparent_ok {
                    return None;
                }
                let mask = match mode {
                    2 => 0x3F,
                    3 => 0x7F,
                    _ => 0xFF,
                };
                Some((d.colr & !mask) | (t & mask))
            }
            _ => {
                // RGB: 0x0000-0x3FFF transparent
                let t = rd16(self.vram, (src + (r * tw + u) * 2) & 0x7_FFFE);
                if t & 0xC000 == 0 && transparent_ok {
                    return None;
                }
                Some(t)
            }
        }
    }
}
