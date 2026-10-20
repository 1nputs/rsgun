"""The SCSP probe suite: small crafted sound logs, each exercising one part of the synthesis, run on
the oracle's SCSP (build/scsp_oracle) and on ours (stv-check scsp): every sample must match.

    python3 tools/scsp/probes.py [NAME...]      # all, or the probes whose names start with NAME

Writes out/scsp-probes/NAME/{writes.bin,samples.bin}; needs build/scsp_oracle (tools/scsp/build.sh)
and target/release/stv-check.
"""
import pathlib
import subprocess
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from probe import ORACLE, ROOT, Probe  # noqa: E402

OUT = ROOT / "out/scsp-probes"
CHECK = ROOT / "target/release/stv-check"
PROBES = {}


def probe(fn):
    PROBES[fn.__name__] = fn
    return fn


def wave_const(p, v=0x4000, n=64):
    p.ram16(0, [v] * n)


def ramp(p, n=512, scale=64):
    p.ram16(0, [(i * scale) & 0xFFFF for i in range(n)])


def noise_wave(p, n=0x800, seed=1):
    x, words = seed, []
    for _ in range(n):
        x = (x * 1103515245 + 12345) & 0x7FFFFFFF
        words.append((x >> 8) & 0xFFFF)
    p.ram16(0, words)
    return words


# ---- levels -----------------------------------------------------------------------------------

@probe
def levels_tl():
    p = Probe()
    wave_const(p)
    p.slot(0, lsa=0, lea=64, loop=1)
    p.key_on([0], at=4)
    for k, tl in enumerate([1, 5, 16, 37, 64, 100, 128, 200, 255]):
        p.set(0, 6, tl, at=100 + 40 * k)
    return p, 500


@probe
def levels_values():
    p = Probe()
    noise_wave(p)
    p.slot(0, lsa=0, lea=0x7FF, loop=1, tl=3)
    p.key_on([0], at=0)
    return p, 3000


@probe
def levels_pan_sdl():
    p = Probe()
    noise_wave(p)
    p.slot(0, lsa=0, lea=0x7FF, loop=1, tl=0, disdl=7, dipan=0)
    p.key_on([0], at=0)
    t = 40
    for sdl in range(8):
        for pan in [0, 1, 3, 7, 0x10, 0x13, 0x1E, 0x0F]:
            p.set(0, 11, (sdl << 13) | (pan << 8), at=t)
            t += 20
    return p, t + 20


@probe
def levels_mvol():
    p = Probe(mvol=15)
    noise_wave(p)
    p.slot(0, lsa=0, lea=0x7FF, loop=1)
    p.slot(1, sa=0x100, lsa=0, lea=0x700, loop=1, dipan=0x14)
    p.key_on([0, 1], at=0)
    for k, m in enumerate(range(15, -1, -1)):
        p.w16(0x400, m, at=40 + 30 * k)
    return p, 600


@probe
def levels_many_slots():
    p = Probe()
    noise_wave(p)
    for n in range(32):
        p.slot(n, sa=2 * 13 * n, lsa=0, lea=0x600, loop=1, tl=n * 3, dipan=n, disdl=7 - (n % 3), fns=n * 17)
    p.key_on(list(range(32)), at=0)
    return p, 2000


# ---- envelopes ----------------------------------------------------------------------------------

@probe
def eg_rates():
    p = Probe()
    wave_const(p, 0x7FFF)
    t = 0
    for n, (ar, d1r, dl, d2r, rr) in enumerate([(31, 20, 10, 15, 25), (25, 12, 20, 5, 14), (18, 31, 4, 9, 31), (12, 3, 31, 0, 6),
                                                (5, 27, 2, 28, 20), (2, 1, 1, 1, 1), (31, 0, 0, 0, 31), (28, 9, 30, 13, 9)]):
        p.slot(n, lsa=0, lea=64, loop=1, ar=ar, d1r=d1r, dl=dl, d2r=d2r, rr=rr, krs=15, dipan=n)
    p.key_on(list(range(8)), at=3)
    p.key_off(list(range(8)), at=9001)
    return p, 14000


@probe
def eg_krs():
    p = Probe()
    wave_const(p, 0x7FFF)
    cases = [(0, 0), (0, 2), (1, -1), (4, -3), (8, 3), (14, 7), (3, -8), (7, 0)]
    for n, (krs, oct_) in enumerate(cases):
        p.slot(n, lsa=0, lea=64, loop=1, ar=14, d1r=11, dl=24, d2r=7, rr=12, krs=krs, oct=oct_ & 15, dipan=n)
    p.key_on(list(range(8)), at=0)
    p.key_off(list(range(8)), at=5000)
    return p, 9000


@probe
def eg_key_timing():
    p = Probe()
    wave_const(p, 0x7FFF)
    for n in range(6):
        p.slot(n, lsa=0, lea=64, loop=1, ar=30, d1r=25, dl=12, d2r=10, rr=22, dipan=2 * n)
    for k, at in enumerate([5, 6, 11, 18, 31]):
        p.key({0: True, 1: k % 2 == 0, 2: True}, at=at * 7)
    p.key_off([0, 1, 2], at=400)
    p.key_on([3, 4], at=401)
    p.key_on([5], at=402)
    p.key_off([3], at=600)
    p.key_on([3], at=603)  # key on again while releasing
    p.key_on([4], at=610)  # and while sounding
    return p, 1500


@probe
def eg_live_changes():
    p = Probe()
    wave_const(p, 0x7FFF)
    p.slot(0, lsa=0, lea=64, loop=1, ar=20, d1r=10, dl=8, d2r=6, rr=8)
    p.key_on([0], at=0)
    p.set(0, 4, (12 << 11) | (25 << 6) | 26, at=101)
    p.set(0, 5, (15 << 10) | (16 << 5) | 3, at=500)
    p.key_off([0], at=1501)
    p.set(0, 5, (15 << 10) | (16 << 5) | 28, at=1700)
    return p, 3000


@probe
def eg_hold_lpslnk():
    p = Probe()
    ramp(p, 600, 40)
    p.slot(0, lsa=200, lea=500, loop=1, ar=10, d1r=20, dl=10, eghold=True)
    p.slot(1, sa=0, lsa=300, lea=500, loop=1, ar=10, d1r=20, dl=10, lpslnk=True, dipan=0x10)
    p.key_on([0, 1], at=0)
    return p, 3000


# ---- pitch and loops ------------------------------------------------------------------------------

@probe
def pitch_steps():
    p = Probe()
    noise_wave(p)
    for n, (oct_, fns) in enumerate([(0, 0), (0, 0x155), (1, 0x2AA), (-1, 0x3FF), (-4, 0x123), (-8, 0x0F0), (3, 0x0FF), (7, 0x001)]):
        p.slot(n, lsa=0, lea=0x7F0, loop=1, oct=oct_ & 15, fns=fns, dipan=n * 3)
    p.key_on(list(range(8)), at=0)
    return p, 4000


@probe
def loops():
    p = Probe()
    noise_wave(p)
    n = 0
    for lp in range(4):
        for oct_, fns in [(0, 0), (0, 0x200), (-2, 0x0AB), (1, 0x321)]:
            p.slot(n, sa=0x40 * n, lsa=37, lea=91, loop=lp, oct=oct_ & 15, fns=fns, dipan=n)
            n += 1
    p.key_on(list(range(n)), at=0)
    return p, 3000


@probe
def loops_8bit():
    p = Probe()
    noise_wave(p)
    for n, lp in enumerate(range(4)):
        p.slot(n, sa=0x101 + 0x80 * n, lsa=20, lea=111, loop=lp, pcm8=True, oct=0, fns=0x1A0 * n, dipan=4 * n)
    for n, sb in enumerate(range(4)):
        p.slot(4 + n, sa=0x400 + 0x40 * n, lsa=0, lea=300, loop=1, sbctl=sb, dipan=0x10 + 4 * n)
        p.slot(8 + n, sa=0x700 + 0x40 * n, lsa=0, lea=300, loop=1, sbctl=sb, pcm8=True, dipan=4 * n)
    p.key_on(list(range(12)), at=0)
    return p, 2000


@probe
def loops_live():
    p = Probe()
    noise_wave(p)
    p.slot(0, lsa=10, lea=100, loop=1)
    p.slot(1, sa=0x200, lsa=10, lea=100, loop=3, dipan=0x10)
    p.key_on([0, 1], at=0)
    p.set(0, 3, 60, at=150)
    p.set(1, 2, 50, at=170)
    p.set(0, 8, (1 << 11) | 0x100, at=300)
    p.set(1, 1, 0x400, at=320)
    return p, 1000


# ---- the DSP ---------------------------------------------------------------------------------------

def dsp_send(p, slot=0, val=None, imxl=7, isel=0, sdir=True, sa=0):
    """A slot sending to MIXS[isel] (no direct output)."""
    if val is None:
        noise_wave(p)
    else:
        wave_const(p, val)
    p.slot(slot, sa=sa, lsa=0, lea=0x7F0 if val is None else 64, loop=1, disdl=0, imxl=imxl, isel=isel, sdir=sdir,
           efsdl=0)


def efreg_out(p, efreg, sdl=7, pan=0):
    p.set(efreg, 11, (sdl << 5) | pan, at=0)


@probe
def dsp_copy():
    p = Probe()
    dsp_send(p)
    p.coef(0, 0xFFF)
    p.dsp_step(0, xsel=1, ysel=1, ira=0x20, zero=1)
    p.dsp_step(1, ewt=1, ewa=0, zero=1)
    efreg_out(p, 0)
    p.key_on([0], at=10)
    return p, 400


@probe
def dsp_multiply():
    p = Probe()
    dsp_send(p)
    for i, c in enumerate([0x7FF, -0x800 & 0x1FFF, 0x123, -0x456 & 0x1FFF, 1, 0x1FFF]):
        p.coef(i, c)
    for i in range(6):
        p.dsp_step(2 * i, xsel=1, ysel=1, ira=0x20, zero=1, coef=i)
        p.dsp_step(2 * i + 1, ewt=1, ewa=i, zero=1, shift=i % 4)
        efreg_out(p, i, pan=0x10 * (i % 2) + i)
    p.key_on([0], at=0)
    return p, 600


@probe
def dsp_temp_delay():
    p = Probe()
    dsp_send(p)
    p.coef(0, 0xFFF)
    # TEMP[DEC] <- MIXS0; EFREG0 <- TEMP[DEC + 5]; EFREG1 <- TEMP[DEC + 100]
    p.dsp_step(0, xsel=1, ysel=1, ira=0x20, zero=1)
    p.dsp_step(1, twt=1, twa=0, zero=1)
    p.dsp_step(2, tra=5, ysel=1, zero=1)
    p.dsp_step(3, ewt=1, ewa=0, zero=1)
    p.dsp_step(4, tra=100, ysel=1, zero=1)
    p.dsp_step(5, ewt=1, ewa=1, zero=1)
    efreg_out(p, 0)
    efreg_out(p, 1, pan=0x1F)
    p.key_on([0], at=0)
    return p, 800


@probe
def dsp_memory():
    p = Probe()
    dsp_send(p)
    p.w16(0x402, (1 << 7) | 0x30)  # RBL 16K words, RBP 0x30 (in 4K-word units)
    p.coef(0, 0xFFF)
    p.w16(0x780, 0x0000)  # MADRS 0: write here
    p.w16(0x782, 0x0011)  # MADRS 1: read 17 samples back
    p.w16(0x784, 0x0300)
    p.dsp_step(0, xsel=1, ysel=1, ira=0x20, zero=1)
    p.dsp_step(1, mwt=1, masa=0, zero=1)                    # float write
    p.dsp_step(3, mrd=1, masa=1, zero=1)                    # float read
    p.dsp_step(5, iwt=1, iwa=4, zero=1)                     # MEMS4 <- it
    p.dsp_step(6, xsel=1, ysel=1, ira=4, zero=1)
    p.dsp_step(7, ewt=1, ewa=0, zero=1, mwt=1, masa=2, nofl=1)  # EFREG0 <- it; and a raw write
    p.dsp_step(9, mrd=1, masa=2, nofl=1, zero=1)            # raw read
    p.dsp_step(11, iwt=1, iwa=5, zero=1)
    p.dsp_step(12, xsel=1, ysel=1, ira=5, zero=1)
    p.dsp_step(13, ewt=1, ewa=1, zero=1)
    efreg_out(p, 0)
    efreg_out(p, 1, pan=0x1F)
    p.key_on([0], at=0)
    return p, 1200


@probe
def dsp_game_program():
    """The game's echo program (play1's log at 26.97 s) on a sent noise."""
    p = Probe(mvol=7)
    dsp_send(p, imxl=6)
    p.w16(0x402, (1 << 7) | 0x06)
    coef = [4095, 2048, 0, -2048, -4095, 1675, 1675, 744, 1433, 2048, 0, 4095, 4095, 2048, 1990, -3981, 1990, 3980, -1934]
    for i, c in enumerate(coef):
        p.coef(i, c & 0x1FFF)
    for i, m in enumerate([0x0000, 0x2274, 0x279f]):
        p.w16(0x780 + 2 * i, m)
    prog = {1: "0000 0000 2000 0004", 3: "0000 0020 2000 0005", 4: "0100 2000 0002 0e00", 5: "0000 a021 2001 0a08",
            6: "0000 a040 0001 0c00", 7: "0280 2022 2002 0e09", 8: "0000 a080 0001 0a00", 9: "0000 a0e3 0001 0c00",
            10: "0081 2000 0002 1000", 11: "0100 2000 0001 1000", 12: "7c00 2000 0001 0000", 13: "0000 a000 4002 1200",
            14: "7c00 2000 0001 1400", 15: "00ff a080 0002 1200", 16: "7c00 2000 0001 1400", 17: "00fe 0000 0000 0000",
            18: "7f00 2000 0002 1600", 19: "0000 0000 1000 0000", 20: "7e00 2000 0002 1800", 21: "0000 0000 1100 0000",
            22: "0000 a800 0002 1a00", 23: "00fd 0000 0000 0000", 24: "7d00 2000 0002 0000", 25: "0583 2000 0002 2000",
            26: "0400 2000 0001 1e00", 27: "0300 2000 0001 1c00", 28: "0700 2000 0001 2200", 29: "0800 2000 0001 2400",
            30: "0386 2000 0011 0400", 31: "00fc 0000 0010 0000"}
    for st, words in prog.items():
        for k, w in enumerate(words.split()):
            p.w16(0x800 + 8 * st + 2 * k, int(w, 16))
    efreg_out(p, 0, pan=0x0F)
    efreg_out(p, 1, pan=0x11)
    p.key_on([0], at=0)
    p.key_off([0], at=20000)
    return p, 60000


# ---- running ----------------------------------------------------------------------------------------

def main():
    names = [n for n in PROBES if not sys.argv[1:] or any(n.startswith(a) for a in sys.argv[1:])]
    if not ORACLE.exists():
        sys.exit("build/scsp_oracle is missing: tools/scsp/build.sh")
    failed = 0
    for name in names:
        p, samples = PROBES[name]()
        d = OUT / name
        d.mkdir(parents=True, exist_ok=True)
        (d / "writes.bin").write_bytes(p.log())
        subprocess.run([str(ORACLE), str(d / "writes.bin"), str(d / "samples.bin"), str(samples)], check=True)
        r = subprocess.run([str(CHECK), "scsp", str(d / "writes.bin")], capture_output=True, text=True)
        lines = r.stdout.strip().splitlines()
        print(f"{name:20s} {'ok' if r.returncode == 0 else 'DIFFERS'}: {lines[0] if lines else r.stderr.strip()}")
        if r.returncode != 0:
            failed += 1
            for line in lines[1:6]:
                print("    " + line)
    print(f"{len(names) - failed} of {len(names)} probes match")
    sys.exit(1 if failed else 0)


main()
