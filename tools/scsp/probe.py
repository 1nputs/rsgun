"""Crafted SCSP sound logs, run on the oracle's SCSP (build/scsp_oracle), to measure one part of the
synthesis at a time. A probe is a list of writes at sample stamps; the result is the samples.

    from probe import Probe
    p = Probe()
    p.ram16(0, [0x4000] * 64)                 # a waveform
    p.slot(0, sa=0, lsa=0, lea=64, loop=1, ar=31, tl=0, disdl=7)
    p.key_on([0], at=10)
    left, right = p.run(200)

The log format is the oracle's sound log (tools/patches/mednafen/oracle.inc): 16-byte records.
"""
import array
import pathlib
import struct
import subprocess
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[2]
ORACLE = ROOT / "build/scsp_oracle"
REGS = 0x100000


class Probe:
    def __init__(self, mvol: int = 15):
        self.recs: list[tuple[int, int, int, int, int]] = []
        self.at = 0
        self.w16(0x400, mvol & 0xF)

    def w16(self, a: int, v: int, at: int | None = None) -> None:
        """A 16-bit register write (a: offset in the register space, 0x000-0xFFF)."""
        self.recs.append((self.at if at is None else at, ord("W"), 2, REGS | a, v & 0xFFFF))

    def ram16(self, a: int, words: list[int], at: int = 0) -> None:
        for i, w in enumerate(words):
            self.recs.append((at, ord("W"), 2, a + 2 * i, w & 0xFFFF))

    def ram8(self, a: int, data: bytes, at: int = 0) -> None:
        for i, b in enumerate(data):
            self.recs.append((at, ord("W"), 1, a + i, b))

    def slot(self, n: int, *, sa=0, lsa=0, lea=0, loop=0, pcm8=False, ssctl=0, sbctl=0, ar=31, d1r=0, d2r=0,
             eghold=False, lpslnk=False, krs=15, dl=0, rr=31, stwinh=False, sdir=False, tl=0, mdl=0, mdxsl=0,
             mdysl=0, oct=0, fns=0, lfore=False, lfof=0, plfows=0, plfos=0, alfows=0, alfos=0, isel=0, imxl=0,
             disdl=7, dipan=0, efsdl=0, efpan=0, at: int | None = None) -> None:
        """Every register of slot n (KYONB clear)."""
        b = 0x20 * n
        regs = [
            (sbctl << 9) | (ssctl << 7) | (loop << 5) | (int(pcm8) << 4) | ((sa >> 16) & 0xF),
            sa & 0xFFFF,
            lsa,
            lea,
            (d2r << 11) | (d1r << 6) | (int(eghold) << 5) | ar,
            (int(lpslnk) << 14) | (krs << 10) | (dl << 5) | rr,
            (int(stwinh) << 9) | (int(sdir) << 8) | tl,
            (mdl << 12) | (mdxsl << 6) | mdysl,
            ((oct & 0xF) << 11) | fns,
            (int(lfore) << 15) | (lfof << 10) | (plfows << 8) | (plfos << 5) | (alfows << 3) | alfos,
            (isel << 3) | imxl,
            (disdl << 13) | (dipan << 8) | (efsdl << 5) | efpan,
        ]
        for i, v in enumerate(regs):
            self.w16(b + 2 * i, v, at)

    def set(self, n: int, reg: int, v: int, at: int) -> None:
        """One slot register (its index 0-11) at a stamp."""
        self.w16(0x20 * n + 2 * reg, v, at)

    def dsp_step(self, n: int, *, tra=0, twt=0, twa=0, xsel=0, ysel=0, ira=0, iwt=0, iwa=0, table=0, mwt=0, mrd=0,
                 ewt=0, ewa=0, adrl=0, frcl=0, shift=0, yrl=0, negb=0, zero=0, bsel=0, nofl=0, coef=0, masa=0, adreb=0,
                 nxadr=0, at: int = 0) -> None:
        """DSP step n's four MPRO words."""
        words = [
            (tra << 8) | (twt << 7) | twa,
            (xsel << 15) | (ysel << 13) | (ira << 6) | (iwt << 5) | iwa,
            (table << 15) | (mwt << 14) | (mrd << 13) | (ewt << 12) | (ewa << 8) | (adrl << 7) | (frcl << 6) | (shift << 4)
            | (yrl << 3) | (negb << 2) | (zero << 1) | bsel,
            (nofl << 15) | (coef << 9) | (masa << 2) | (adreb << 1) | nxadr,
        ]
        for i, w in enumerate(words):
            self.w16(0x800 + 8 * n + 2 * i, w, at)

    def coef(self, i: int, v: int, at: int = 0) -> None:
        """COEF i (13 bits, as written: shifted up 3)."""
        self.w16(0x700 + 2 * i, v << 3, at)

    def key(self, slots: dict[int, bool], at: int, base: dict[int, int] | None = None) -> None:
        """KYONB for the given slots, then KYONEX (through the first slot's register 0). `base`: each
        slot's register 0 without KYONB (from slot()); read back from the log when not given."""
        for n, on in slots.items():
            r0 = self.reg0(n) if base is None else base[n]
            self.w16(0x20 * n, (r0 & 0x07FF) | (0x800 if on else 0), at)
        n = next(iter(slots))
        r0 = self.reg0(n) if base is None else base[n]
        self.w16(0x20 * n, (r0 & 0x07FF) | (0x800 if slots[n] else 0) | 0x1000, at)

    def key_on(self, slots: list[int], at: int) -> None:
        self.key({n: True for n in slots}, at)

    def key_off(self, slots: list[int], at: int) -> None:
        self.key({n: False for n in slots}, at)

    def reg0(self, n: int) -> int:
        v = 0
        for r in self.recs:
            if r[3] == REGS | (0x20 * n):
                v = r[4]
        return v

    def log(self) -> bytes:
        recs = sorted(enumerate(self.recs), key=lambda ir: (ir[1][0], ir[0]))
        return b"".join(struct.pack(">IBBxxII", s, t, z, a, v) for _, (s, t, z, a, v) in recs)

    def run(self, samples: int, ram: bool = False):
        """The oracle's samples: (left, right), and with `ram` sound RAM at the end (words)."""
        with tempfile.TemporaryDirectory() as d:
            w, o, r = pathlib.Path(d) / "writes.bin", pathlib.Path(d) / "out.bin", pathlib.Path(d) / "ram.bin"
            w.write_bytes(self.log())
            subprocess.run([str(ORACLE), str(w), str(o), str(samples), str(r)], check=True)
            a = array.array("h")
            a.frombytes(o.read_bytes())
            a.byteswap()
            m = array.array("H")
            m.frombytes(r.read_bytes())
            m.byteswap()
        if ram:
            return list(a[0::2]), list(a[1::2]), list(m)
        return list(a[0::2]), list(a[1::2])
