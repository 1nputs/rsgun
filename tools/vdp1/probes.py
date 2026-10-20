"""The VDP1 probe suite: crafted command lists drawn by the oracle's VDP1 (build/vdp1_oracle) and by
ours (stv-check vdp1): every framebuffer pixel must match.

    python3 tools/vdp1/probes.py [NAME...]       # all, or the probes whose names start with NAME

Writes out/vdp1-probes/NAME/draw/ in the oracle's draw-dump layout (vdp1-vram.bin, fb-before.bin,
vdp1-regs.bin, fb-after.bin from the oracle's VDP1); needs build/vdp1_oracle (tools/vdp1/build.sh)
and target/release/stv-check. Textures are RGB with each texel's (u, v) in its bits, so a pixel
shows which texel was drawn there.
"""
import pathlib
import random
import shutil
import struct
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
ORACLE = ROOT / "build/vdp1_oracle"
CHECK = ROOT / "target/release/stv-check"
OUT = ROOT / "out/vdp1-probes"
PROBES = {}

# command types
NORMAL, SCALED, DISTORTED, POLYGON, POLYLINE, LINE, USER_CLIP, SYSTEM_CLIP, LOCAL = 0, 1, 2, 4, 5, 6, 8, 9, 10


def probe(fn):
    PROBES[fn.__name__] = fn
    return fn


class Draw:
    def __init__(self, tvmr=0, fbcr=0):
        self.vram = bytearray(0x80000)
        self.fb = bytearray(0x40000)
        self.regs = [tvmr, fbcr, 1, 0, 0, 0]
        self.cmds = []
        self.tex_next = 0x20000  # textures from here (bytes)
        self.cmd(SYSTEM_CLIP, xc=511, yc=255)
        self.cmd(LOCAL, xa=0, ya=0)

    def cmd(self, comm, *, jp=0, zp=0, dir=0, pmod=0, colr=0, srca=0, size=(0, 0), xa=0, ya=0, xb=0, yb=0, xc=0, yc=0,
            xd=0, yd=0, grda=0):
        w = [(jp << 12) | (zp << 8) | (dir << 4) | comm, 0, pmod, colr, srca // 8, ((size[0] // 8) << 8) | size[1],
             xa & 0xFFFF, ya & 0xFFFF, xb & 0xFFFF, yb & 0xFFFF, xc & 0xFFFF, yc & 0xFFFF, xd & 0xFFFF, yd & 0xFFFF,
             grda // 8, 0]
        self.cmds.append(w)

    def texture_uv(self, w, h):
        """An RGB texture whose texel (u, v) is 0x8000 | u | v << 6 (u, v below 64): its address."""
        a = self.tex_next
        for v in range(h):
            for u in range(w):
                struct.pack_into(">H", self.vram, a + 2 * (v * w + u), 0x8000 | (u & 0x3F) | ((v & 0x3F) << 6))
        self.tex_next = (a + 2 * w * h + 31) & ~31
        return a

    def gouraud(self, colors):
        a = self.tex_next
        for i, c in enumerate(colors):
            struct.pack_into(">H", self.vram, a + 2 * i, c)
        self.tex_next = (a + 8 + 31) & ~31
        return a

    def fill_fb(self, f):
        for y in range(256):
            for x in range(512):
                struct.pack_into(">H", self.fb, 2 * (y * 512 + x), f(x, y))

    def finish(self):
        for i, w in enumerate(self.cmds + [[0x8000] + [0] * 15]):
            for k, v in enumerate(w):
                struct.pack_into(">H", self.vram, 32 * i + 2 * k, v)


RGB_SPD = (5 << 3) | 0x40 | 0x80  # RGB texels, transparent pixels drawn, no end codes


@probe
def untextured_opacity():
    """Untextured commands without SPD: drawn or not by VRAM's last dot in their colour mode."""
    d = Draw()
    d.vram[0x7FFFE], d.vram[0x7FFFF] = 0x40, 0x0F
    for k, pmod in enumerate([0, 0x08, 0x10, 0x18, 0x20, 0x28, 0x30, 0x38, 0x80, 0xA8, 0x40]):
        d.cmd(LINE, pmod=pmod, colr=0x8000 | (k * 999), xa=10, ya=10 + 6 * k, xb=60, yb=14 + 6 * k)
        d.cmd(POLYGON, pmod=pmod, colr=0x8000 | (k * 999), xa=80, ya=10 + 6 * k, xb=120, yb=10 + 6 * k, xc=120, yc=14 + 6 * k,
              xd=80, yd=14 + 6 * k)
    return d


# ---- lines and polygons -------------------------------------------------------------------------

@probe
def lines_octants():
    d = Draw()
    for k, (dx, dy) in enumerate([(a, b) for a in range(-12, 13, 3) for b in range(-12, 13, 3)]):
        cx, cy = 20 + (k % 9) * 28, 20 + (k // 9) * 28
        d.cmd(LINE, pmod=0x40, colr=0x8000 | (k + 1), xa=cx, ya=cy, xb=cx + dx, yb=cy + dy)
    return d


@probe
def lines_random():
    d, r = Draw(), random.Random(1)
    for k in range(300):
        x0, y0 = r.randrange(-20, 530), r.randrange(-20, 270)
        x1, y1 = x0 + r.randrange(-60, 61), y0 + r.randrange(-60, 61)
        d.cmd(LINE, pmod=0x40, colr=0x8000 | k, xa=x0, ya=y0, xb=x1, yb=y1)
    return d


@probe
def polygons():
    d, r = Draw(), random.Random(2)
    for k in range(60):
        cx, cy = r.randrange(0, 512), r.randrange(0, 256)
        pts = [(cx + r.randrange(-30, 31), cy + r.randrange(-30, 31)) for _ in range(4)]
        d.cmd(POLYGON, pmod=0x40, colr=0x8000 | (k * 37), xa=pts[0][0], ya=pts[0][1], xb=pts[1][0], yb=pts[1][1], xc=pts[2][0],
              yc=pts[2][1], xd=pts[3][0], yd=pts[3][1])
    return d


@probe
def polylines():
    d, r = Draw(), random.Random(3)
    for k in range(40):
        cx, cy = r.randrange(0, 512), r.randrange(0, 256)
        pts = [(cx + r.randrange(-40, 41), cy + r.randrange(-40, 41)) for _ in range(4)]
        d.cmd(POLYLINE, pmod=0x40, colr=0x8000 | (k * 91), xa=pts[0][0], ya=pts[0][1], xb=pts[1][0], yb=pts[1][1], xc=pts[2][0],
              yc=pts[2][1], xd=pts[3][0], yd=pts[3][1])
    return d


@probe
def polygons_rotated():
    """Rotated rectangles as polygons: long edges at many slopes."""
    import math
    d = Draw()
    for k in range(24):
        cx, cy = 30 + (k % 8) * 62, 40 + (k // 8) * 80
        ang = k * math.pi / 24
        hw, hh = 12 + k, 9 + (k % 5) * 4
        pts = []
        for sx, sy in [(-1, -1), (1, -1), (1, 1), (-1, 1)]:
            x, y = sx * hw, sy * hh
            pts.append((round(cx + x * math.cos(ang) - y * math.sin(ang)), round(cy + x * math.sin(ang) + y * math.cos(ang))))
        d.cmd(POLYGON, pmod=0x40, colr=0x8000 | (k * 1111 & 0x7FFF), xa=pts[0][0], ya=pts[0][1], xb=pts[1][0], yb=pts[1][1],
              xc=pts[2][0], yc=pts[2][1], xd=pts[3][0], yd=pts[3][1])
    return d


# ---- sprites ------------------------------------------------------------------------------------------

@probe
def normal_sprites():
    d = Draw()
    for k, (w, h) in enumerate([(8, 1), (8, 8), (16, 5), (24, 24), (64, 40), (8, 63)]):
        t = d.texture_uv(w, h)
        for f in range(4):
            d.cmd(NORMAL, dir=f, pmod=RGB_SPD, srca=t, size=(w, h), xa=10 + 80 * k, ya=5 + 62 * f)
    return d


@probe
def scaled_sprites():
    d, r = Draw(), random.Random(4)
    t = d.texture_uv(16, 16)
    t2 = d.texture_uv(40, 24)
    for k in range(40):
        tex, (w, h) = (t, (16, 16)) if k % 2 == 0 else (t2, (40, 24))
        x, y = r.randrange(0, 480), r.randrange(0, 230)
        zw, zh = r.randrange(1, 70), r.randrange(1, 60)
        if k < 20:
            d.cmd(SCALED, dir=k % 4, pmod=RGB_SPD, srca=tex, size=(w, h), xa=x, ya=y, xc=x + zw, yc=y + zh)
        else:
            zp = [5, 6, 7, 9, 10, 11, 13, 14, 15][k % 9]
            d.cmd(SCALED, zp=zp, dir=k % 4, pmod=RGB_SPD, srca=tex, size=(w, h), xa=x, ya=y, xb=zw - 30, yb=zh - 25)
    return d


@probe
def distorted_sprites():
    d, r = Draw(), random.Random(5)
    texs = [((w, h), d.texture_uv(w, h)) for (w, h) in [(8, 8), (16, 16), (32, 24), (64, 64), (24, 7)]]
    for k in range(60):
        (w, h), t = texs[k % len(texs)]
        cx, cy = r.randrange(0, 512), r.randrange(0, 256)
        s = r.choice([6, 15, 30, 50])
        pts = [(cx + r.randrange(-s, s + 1), cy + r.randrange(-s, s + 1)) for _ in range(4)]
        d.cmd(DISTORTED, dir=k % 4, pmod=RGB_SPD, srca=t, size=(w, h), xa=pts[0][0], ya=pts[0][1], xb=pts[1][0],
              yb=pts[1][1], xc=pts[2][0], yc=pts[2][1], xd=pts[3][0], yd=pts[3][1])
    return d


@probe
def distorted_rects():
    """Axis-aligned and rotated rectangles: the shapes games draw most."""
    d = Draw()
    t = d.texture_uv(32, 32)
    import math
    for k in range(24):
        cx, cy = 30 + (k % 8) * 62, 40 + (k // 8) * 80
        ang = k * math.pi / 24
        hw, hh = 12 + k, 9 + (k % 5) * 4
        pts = []
        for sx, sy in [(-1, -1), (1, -1), (1, 1), (-1, 1)]:
            x, y = sx * hw, sy * hh
            pts.append((round(cx + x * math.cos(ang) - y * math.sin(ang)), round(cy + x * math.sin(ang) + y * math.cos(ang))))
        d.cmd(DISTORTED, pmod=RGB_SPD, srca=t, size=(32, 32), xa=pts[0][0], ya=pts[0][1], xb=pts[1][0], yb=pts[1][1],
              xc=pts[2][0], yc=pts[2][1], xd=pts[3][0], yd=pts[3][1])
    return d


# ---- colour modes, end codes, shading, clipping ---------------------------------------------------------

def texture_4bpp(d, w, h, seed, ends=0):
    """A 4bpp texture of random dots (some 0, transparent; `ends` end codes 0xF placed at random)."""
    r = random.Random(seed)
    a = d.tex_next
    for v in range(h):
        for u in range(0, w, 2):
            hi, lo = r.randrange(0, 15), r.randrange(0, 15)
            d.vram[a + (v * w + u) // 2] = (hi << 4) | lo
    for _ in range(ends):
        i = r.randrange(0, w * h)
        b = a + i // 2
        d.vram[b] = (d.vram[b] & 0x0F) | 0xF0 if i % 2 == 0 else (d.vram[b] & 0xF0) | 0x0F
    d.tex_next = (a + w * h // 2 + 31) & ~31
    return a


def texture_8bpp(d, w, h, seed, ends=0):
    r = random.Random(seed)
    a = d.tex_next
    for i in range(w * h):
        d.vram[a + i] = r.randrange(0, 255)
    for _ in range(ends):
        d.vram[a + r.randrange(0, w * h)] = 0xFF
    d.tex_next = (a + w * h + 31) & ~31
    return a


@probe
def colour_modes():
    d = Draw()
    lut = d.tex_next
    for i in range(16):
        struct.pack_into(">H", d.vram, lut + 2 * i, 0x8000 | (i * 0x0842))
    d.tex_next += 64
    t4 = texture_4bpp(d, 16, 16, 1)
    t4e = texture_4bpp(d, 16, 16, 2, ends=12)
    t8 = texture_8bpp(d, 16, 16, 3)
    t8e = texture_8bpp(d, 16, 16, 4, ends=12)
    k = 0
    for tex, modes in [(t4, [0, 1]), (t4e, [0, 1]), (t8, [2, 3, 4]), (t8e, [2, 3, 4])]:
        for mode in modes:
            for flags in [0, 0x40, 0x80, 0xC0]:
                colr = (lut // 8) if mode == 1 else 0x1230
                x, y = 10 + (k % 16) * 30, 10 + (k // 16) * 40
                d.cmd(NORMAL, pmod=(mode << 3) | flags, colr=colr, srca=tex, size=(16, 16), xa=x, ya=y)
                d.cmd(DISTORTED, dir=k % 4, pmod=(mode << 3) | flags, colr=colr, srca=tex, size=(16, 16), xa=x, ya=y + 18,
                      xb=x + 22, yb=y + 19, xc=x + 20, yc=y + 33, xd=x + 1, yd=y + 30)
                k += 1
    return d


@probe
def gouraud():
    d, r = Draw(), random.Random(6)
    t = d.texture_uv(16, 16)
    lut = d.tex_next
    for i in range(16):
        struct.pack_into(">H", d.vram, lut + 2 * i, 0x8000 | (i * 0x0842))
    d.tex_next += 64
    t4 = texture_4bpp(d, 16, 16, 7)
    for k in range(40):
        g = d.gouraud([r.randrange(0, 0x8000) for _ in range(4)])
        cx, cy = r.randrange(0, 512), r.randrange(0, 256)
        pts = [(cx + r.randrange(-25, 26), cy + r.randrange(-25, 26)) for _ in range(4)]
        kind = k % 4
        if kind == 0:
            d.cmd(POLYGON, pmod=0x40 | 4, colr=0x8000 | r.randrange(0, 0x8000), grda=g, xa=pts[0][0], ya=pts[0][1],
                  xb=pts[1][0], yb=pts[1][1], xc=pts[2][0], yc=pts[2][1], xd=pts[3][0], yd=pts[3][1])
        elif kind == 1:
            d.cmd(DISTORTED, pmod=RGB_SPD | 4, srca=t, size=(16, 16), grda=g, xa=pts[0][0], ya=pts[0][1], xb=pts[1][0],
                  yb=pts[1][1], xc=pts[2][0], yc=pts[2][1], xd=pts[3][0], yd=pts[3][1])
        elif kind == 2:
            d.cmd(DISTORTED, pmod=(1 << 3) | 0x80 | 4, colr=lut // 8, srca=t4, size=(16, 16), grda=g, xa=pts[0][0],
                  ya=pts[0][1], xb=pts[1][0], yb=pts[1][1], xc=pts[2][0], yc=pts[2][1], xd=pts[3][0], yd=pts[3][1])
        else:
            d.cmd(SCALED, pmod=RGB_SPD | 4, srca=t, size=(16, 16), grda=g, xa=pts[0][0], ya=pts[0][1], xc=pts[0][0] + 30,
                  yc=pts[0][1] + 20)
    return d


@probe
def high_speed_shrink():
    d, r = Draw(), random.Random(8)
    t = d.texture_uv(32, 32)
    for k in range(40):
        cx, cy = r.randrange(0, 512), r.randrange(0, 256)
        s = r.choice([5, 10, 16, 30])
        pts = [(cx + r.randrange(-s, s + 1), cy + r.randrange(-s, s + 1)) for _ in range(4)]
        if k % 2:
            d.cmd(DISTORTED, dir=k % 4, pmod=RGB_SPD | 0x1000, srca=t, size=(32, 32), xa=pts[0][0], ya=pts[0][1],
                  xb=pts[1][0], yb=pts[1][1], xc=pts[2][0], yc=pts[2][1], xd=pts[3][0], yd=pts[3][1])
        else:
            d.cmd(SCALED, dir=k % 4, pmod=RGB_SPD | 0x1000, srca=t, size=(32, 32), xa=cx, ya=cy, xc=cx + r.randrange(2, 40),
                  yc=cy + r.randrange(2, 40))
    return d


@probe
def mesh_and_clip():
    d, r = Draw(), random.Random(9)
    t = d.texture_uv(16, 16)
    d.cmd(USER_CLIP, xa=100, ya=50, xc=300, yc=180)
    for k in range(40):
        cx, cy = r.randrange(50, 350), r.randrange(20, 210)
        pts = [(cx + r.randrange(-30, 31), cy + r.randrange(-30, 31)) for _ in range(4)]
        pmod = RGB_SPD | [0x100, 0x400, 0x600, 0x500][k % 4]
        if k % 3 == 0:
            d.cmd(POLYGON, pmod=0x40 | (pmod & 0x700), colr=0x8000 | (k * 811), xa=pts[0][0], ya=pts[0][1], xb=pts[1][0],
                  yb=pts[1][1], xc=pts[2][0], yc=pts[2][1], xd=pts[3][0], yd=pts[3][1])
        else:
            d.cmd(DISTORTED, pmod=pmod, srca=t, size=(16, 16), xa=pts[0][0], ya=pts[0][1], xb=pts[1][0], yb=pts[1][1],
                  xc=pts[2][0], yc=pts[2][1], xd=pts[3][0], yd=pts[3][1])
    d.cmd(SYSTEM_CLIP, xc=200, yc=120)
    for k in range(10):
        d.cmd(POLYGON, pmod=0x40, colr=0x8000 | (k * 2111), xa=150 + k * 8, ya=90, xb=240 + k * 8, yb=95, xc=230, yc=150,
              xd=140, yd=140)
    return d


@probe
def colour_calc():
    d, r = Draw(), random.Random(10)
    d.fill_fb(lambda x, y: (0x8000 | ((x * 7) & 0x1F) | ((y & 0x1F) << 5) | (((x + y) & 0x1F) << 10)) if (x // 16 + y // 16) % 3 else ((x * 5) & 0x7FF))
    t = d.texture_uv(16, 16)
    for k in range(48):
        cx, cy = r.randrange(0, 500), r.randrange(0, 240)
        pmod = RGB_SPD | [0, 1, 2, 3, 0x8000, 4 | 3, 4 | 2, 1][k % 8]
        g = d.gouraud([r.randrange(0, 0x8000) for _ in range(4)]) if pmod & 4 else 0
        if k % 2:
            d.cmd(NORMAL, pmod=pmod, srca=t, size=(16, 16), grda=g, xa=cx, ya=cy)
        else:
            d.cmd(POLYGON, pmod=0x40 | (pmod & 0x8007), colr=0x8000 | (k * 777), grda=g, xa=cx, ya=cy, xb=cx + 20, yb=cy + 3,
                  xc=cx + 18, yc=cy + 22, xd=cx + 2, yd=cy + 19)
    return d


# ---- running ---------------------------------------------------------------------------------------------

def main():
    names = [n for n in PROBES if not sys.argv[1:] or any(n.startswith(a) for a in sys.argv[1:])]
    if not ORACLE.exists():
        sys.exit("build/vdp1_oracle is missing: tools/vdp1/build.sh")
    failed = 0
    for name in names:
        d = PROBES[name]()
        d.finish()
        dd = OUT / name / "draw"
        dd.mkdir(parents=True, exist_ok=True)
        (dd / "vdp1-vram.bin").write_bytes(d.vram)
        (dd / "fb-before.bin").write_bytes(d.fb)
        (dd / "vdp1-regs.bin").write_bytes(b"".join(struct.pack(">H", v) for v in d.regs))
        subprocess.run([str(ORACLE), str(dd)], check=True)
        shutil.move(dd / "fb-oracle.bin", dd / "fb-after.bin")
        r = subprocess.run([str(CHECK), "vdp1", str(OUT / name)], capture_output=True, text=True)
        out = r.stdout.strip() or r.stderr.strip()
        ok = r.returncode == 0
        failed += not ok
        print(f"{name:20s} {'ok' if ok else 'DIFFERS'}: {out.split(': ', 1)[1] if ': ' in out else out}"[:220])
    print(f"{len(names) - failed} of {len(names)} probes match")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
