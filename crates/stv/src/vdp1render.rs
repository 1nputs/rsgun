//! VDP1's drawing: a command list in VRAM drawn into a framebuffer (512 x 256, 16-bit pixels). From
//! the VDP1 user's manual; the rasterization's details are checked against the oracle's draws
//! (stv-check vdp1: its framebuffer before and after a draw).
//!
//! Quadrilaterals (sprites and polygons) are drawn as VDP1 draws them: the left edge A-D and the
//! right edge B-C are stepped together over the longer edge's length, and a line is drawn from the
//! one to the other at every step, the texture's row following the step and its column the line;
//! a line adds a pixel wherever both coordinates step at once, so no gaps open between lines.

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
                    let x0 = match zp & 3 {
                        2 => ax - w / 2,
                        3 => ax - w,
                        _ => ax,
                    };
                    let y0 = match (zp >> 2) & 3 {
                        2 => ay - h / 2,
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
            4 => {
                let v = [0, 1, 2, 3].map(|i| self.vertex(a, i));
                self.quad(v, Draw { tex: None, ..tex });
            }
            5 | 7 => {
                let v = [0, 1, 2, 3].map(|i| self.vertex(a, i));
                for i in 0..4 {
                    self.line(v[i], v[(i + 1) % 4], None, Draw { tex: None, ..tex }, None);
                }
            }
            6 => {
                let (p0, p1) = (self.vertex(a, 0), self.vertex(a, 1));
                self.line(p0, p1, None, Draw { tex: None, ..tex }, None);
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
            let lerp = |p: (i32, i32), q: (i32, i32)| -> (i32, i32) {
                if n == 0 {
                    p
                } else {
                    (p.0 + (q.0 - p.0) * i / n, p.1 + (q.1 - p.1) * i / n)
                }
            };
            let (l, r) = (lerp(a, dd), lerp(b, c));
            let row = if n == 0 { 0 } else { ((th - 1) * i / n) as usize };
            let g = d.gouraud.map(|g| {
                let mix = |p: [i32; 3], q: [i32; 3]| [0, 1, 2].map(|k| if n == 0 { p[k] } else { p[k] + (q[k] - p[k]) * i / n });
                (mix(g[0], g[3]), mix(g[1], g[2]))
            });
            self.line(l, r, Some(row), d, g);
        }
    }

    /// A line from `p0` to `p1`: texture row `row` across it (or the command's colour), the
    /// gouraud colours at its ends.
    fn line(&mut self, p0: (i32, i32), p1: (i32, i32), row: Option<usize>, d: Draw, g: Option<([i32; 3], [i32; 3])>) {
        let (dx, dy) = (p1.0 - p0.0, p1.1 - p0.1);
        let len = dx.abs().max(dy.abs());
        let (sx, sy) = (dx.signum(), dy.signum());
        let tw = d.tex.map_or(1, |t| t.1) as i32;
        let mut prev: Option<(i32, i32)> = None;
        for j in 0..=len {
            let (x, y) = if len == 0 { p0 } else { (p0.0 + dx * j / len, p0.1 + dy * j / len) };
            let col = if len == 0 { 0 } else { ((tw - 1) * j / len) as usize };
            let gc = g.map(|(a, b)| [0, 1, 2].map(|k| if len == 0 { a[k] } else { a[k] + (b[k] - a[k]) * j / len }));
            if let Some((px, py)) = prev
                && px != x
                && py != y
            {
                // a diagonal step: the extra pixel that closes the gap
                let _ = (sx, sy);
                self.pixel(x, py, row, col, d, gc);
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
                let (u, r) = (if d.hflip { tw - 1 - col } else { col }, if d.vflip { th - 1 - r } else { r });
                let Some(v) = self.texel(src, tw, u, r, d) else { return };
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
                    0x8000 | (((v & 0x7BDE) + (old & 0x7BDE)) >> 1)
                } else {
                    v
                }
            }
            _ => v,
        };
        wr16(self.fb, a, v);
    }

    /// A texel through the command's colour mode; None when it is transparent.
    fn texel(&self, src: usize, tw: usize, u: usize, r: usize, d: Draw) -> Option<u16> {
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
                let t = rd16(self.vram, (src + (r * tw + u) * 2) & 0x7_FFFE);
                if t & 0x8000 == 0 && transparent_ok {
                    return None;
                }
                Some(t)
            }
        }
    }
}
