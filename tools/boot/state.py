#!/usr/bin/env python3
"""Reads a stv save state (crates/stv/src/state.rs, machine.rs) as far as the SCSP's RAM and
registers: the CPUs, work RAM, backup RAM, the SMPC/SCU/on-chip registers, VDP1 and VDP2.
The order of fields is the saved! lists' (little endian; Vec = u64 length + items; Option =
flag + item). Not parsed: the SCSP's slots and the 68000 (after the SCSP's registers).

    state.py STATE            prints the registers
    (import: state.load(path) -> dict)
"""
import struct as _struct
import sys

U8, U16, U32, U64, BOOL = "u8", "u16", "u32", "u64", "bool"


class R:
    def __init__(self, d):
        self.d, self.p = d, 0
        self.marks = {}  # name -> file offset of a byte vector's contents (for editing)

    def take(self, n):
        b = self.d[self.p:self.p + n]
        assert len(b) == n, "state ends early"
        self.p += n
        return b

    def num(self, t):
        n = {U8: 1, U16: 2, U32: 4, U64: 8, BOOL: 1}[t]
        v = int.from_bytes(self.take(n), "little")
        return bool(v) if t == BOOL else v


def rd(r, ty):
    """ty: a base type name, ('arr', ty, n), ('vec', ty), ('opt', ty), ('tup', tys), or a
    ('struct', [(name, ty)...])."""
    if isinstance(ty, str):
        return r.num(ty)
    k = ty[0]
    if k == "arr":
        if ty[1] == U8:
            return r.take(ty[2])
        return [rd(r, ty[1]) for _ in range(ty[2])]
    if k == "vec":
        n = r.num(U64)
        if ty[1] == U8:
            r.last_vec = r.p
            return r.take(n)
        return [rd(r, ty[1]) for _ in range(n)]
    if k == "opt":
        return rd(r, ty[1]) if r.num(BOOL) else None
    if k == "tup":
        return tuple(rd(r, t) for t in ty[1])
    if k == "struct":
        out = {}
        for n, t in ty[1]:
            out[n] = rd(r, t)
            if t == V(U8):
                r.marks[n] = r.last_vec
        return out
    raise ValueError(ty)


def struct(*fields):
    return ("struct", list(fields))


A = lambda t, n: ("arr", t, n)
V = lambda t: ("vec", t)
SH2 = struct(("r", A(U32, 16)), ("pc", U32), ("pr", U32), ("sr", U32), ("gbr", U32), ("vbr", U32),
             ("mach", U32), ("macl", U32), ("delay", ("opt", ("tup", (U32, U32)))),
             ("sleeping", BOOL), ("trapped", BOOL))
FRT = struct(("tier", U8), ("ftcsr", U8), ("frc", U16), ("ocra", U16), ("ocrb", U16), ("tcr", U8),
             ("tocr", U8), ("icr", U16), ("temp", U8), ("sub", U32))
DIVU = struct(*[(n, U32) for n in "dvsr dvdnth dvdntl dvcr vcrdiv".split()])
DMAC = struct(("sar", U32), ("dar", U32), ("tcr", U32), ("chcr", U32), ("vcr", U32), ("drcr", U8))
ONCHIP = struct(("frt", FRT), ("divu", DIVU), ("dma", A(DMAC, 2)), ("dmaor", U32), ("ipra", U16),
                ("iprb", U16), ("vcra", U16), ("vcrb", U16), ("vcrc", U16), ("vcrd", U16),
                ("vcrwdt", U16), ("icr", U16), ("wtcsr", U8), ("wtcnt", U8), ("rstcsr", U8),
                ("sbycr", U8), ("ccr", U8), ("bsc", A(U32, 7)), ("slave", BOOL),
                ("other", V(("tup", (U32, U8)))))
SCUDMA = struct(*[(n, U32) for n in "read write count add enable mode".split()], ("start", BOOL))
SCU = struct(("dma", A(SCUDMA, 3)), ("ims", U32), ("ist", U32), ("t0c", U32), ("t1s", U32),
             ("t1md", U32), ("aiack", U32), ("asr", A(U32, 2)), ("aref", U32), ("rsel", U32),
             ("dsp", A(U32, 4)), ("t0_count", U32), ("t0_matched_line", BOOL))
EEPROM = struct(("mem", A(U16, 64)), ("cs", BOOL), ("sk", BOOL), ("shift", U32), ("bits", U32),
                ("write_enabled", BOOL), ("out", ("opt", ("tup", (U16, U32)))), ("out_addr", U8),
                ("dout", BOOL))
SMPC = struct(("ireg", A(U8, 7)), ("comreg", U8), ("oreg", A(U8, 32)), ("sr", U8), ("sf", U8),
              ("data_out", A(A(U8, 2), 2)), ("data_dir", A(A(U8, 2), 2)), ("direct", A(BOOL, 2)),
              ("exle", A(BOOL, 2)), ("port_state", A(U8, 2)), ("eeprom", EEPROM), ("sctrl", U8),
              ("sctrl_events", V(("tup", (U8, U8)))), ("bus_buffer", U8),
              ("busy", ("opt", ("tup", (U8, U32)))), ("intback_waiting", BOOL), ("area", U8),
              ("dotsel", BOOL), ("reset_enabled", BOOL), ("sound_on", BOOL), ("smem", A(U8, 4)),
              ("rtc_seconds", U64), ("rtc_cycles", U64))
VDP1 = struct(("vram", V(U8)), ("fb", A(V(U8), 2)), ("draw", U64),
              *[(n, U16) for n in "tvmr fbcr ptmr ewdr ewlr ewrr edsr lopr copr".split()],
              ("change_requested", BOOL))
VDP2 = struct(("vram", V(U8)), ("cram", V(U8)), ("regs", V(U8)), ("vblank", BOOL), ("hblank", BOOL),
              ("odd", BOOL))
SCSP_HEAD = struct(("ram", V(U8)), ("regs", V(U8)))
SYS = struct(("wram_l", V(U8)), ("wram_h", V(U8)), ("backup", V(U8)), ("prot", A(U8, 16)),
             ("prot_on", BOOL), ("prot_counter", U8), ("io", A(U8, 0x80)), ("inputs", A(U8, 8)),
             ("onchip", A(ONCHIP, 2)), ("cache_data", A(V(U8), 2)), ("scu", SCU), ("smpc", SMPC),
             ("vdp1", VDP1), ("vdp2", VDP2), ("scsp", SCSP_HEAD))


def offsets(path):
    """File offsets of the byte vectors' contents: wram_l, wram_h, backup, vram, cram, regs..."""
    load(path, marks=True)
    return LAST_MARKS


LAST_MARKS = {}


def load(path, marks=False):
    d = open(path, "rb").read()
    assert d[:8] == b"STVSTATE", "not a save state"
    r = R(d)
    r.take(8)
    version, = _struct.unpack("<I", r.take(4))
    roms = _struct.unpack("<QQ", r.take(16))
    cpu = rd(r, A(SH2, 2))
    sys_ = rd(r, SYS)
    LAST_MARKS.clear()
    LAST_MARKS.update(r.marks)
    sys_.update(tail(d))
    return {"version": version, "roms": roms, "cpu": cpu, "sys": sys_}


TAIL = struct(("sound_on", BOOL), ("sound_reset_pending", BOOL), ("sound_owed", U64),
              ("sound_in_sample", U32), ("sound_steps", U64), ("slave_on", BOOL),
              ("clock352", BOOL), ("line", U32), ("frame", U64), ("reset_pending", BOOL),
              ("nmi_pending", A(BOOL, 2)), ("slave_reset_pending", BOOL), ("wait", U32))
TAIL_SIZE = 1 + 1 + 8 + 4 + 8 + 1 + 1 + 4 + 8 + 1 + 2 + 1 + 4


def tail(d):
    """The Sys fields after the 68000, found from the end: the Machine's picture (w, h, RGB
    bytes) is last, before it steps [u64; 2] and time [u32; 2], before them these."""
    for k in range(24, 24 + 4 * 1024 * 1024, 1):
        p = len(d) - k
        w, h, n = _struct.unpack_from("<QQQ", d, p)
        if 0 < w < 2048 and 0 < h < 2048 and n == p_len(d, p) and n in (w * h * 3, w * h * 4, 0):
            r = R(d)
            r.p = p - 8 - 16 - TAIL_SIZE
            return rd(r, TAIL)
    raise ValueError("no tail")


def p_len(d, p):
    return len(d) - p - 24

TAIL_NAMES = [n for n, _ in TAIL[1]]


def main():
    st = load(sys.argv[1])
    for i, c in enumerate(st["cpu"]):
        print(f"cpu{i}: pc={c['pc']:08x} pr={c['pr']:08x} sr={c['sr']:08x} gbr={c['gbr']:08x} "
              f"vbr={c['vbr']:08x} sleeping={c['sleeping']} delay={c['delay']}")
        print("  r:", " ".join(f"{x:08x}" for x in c["r"]))
    s = st["sys"]
    for i, o in enumerate(s["onchip"]):
        print(f"onchip{i}:", {k: (hex(v) if isinstance(v, int) and not isinstance(v, bool) else v)
                              for k, v in o.items() if k not in ("frt", "divu", "dma", "other", "bsc")})
        print("  bsc", [hex(x) for x in o["bsc"]], "frt", o["frt"], "divu", o["divu"])
        print("  dma", o["dma"], "other", [(hex(a), hex(b)) for a, b in o["other"]])
    print("scu:", {k: (hex(v) if isinstance(v, int) and not isinstance(v, bool) else v)
                   for k, v in s["scu"].items()})
    sm = dict(s["smpc"])
    sm["eeprom"] = "(%d words)" % len(sm["eeprom"]["mem"])
    print("smpc:", {k: (v.hex() if isinstance(v, bytes) else v) for k, v in sm.items()})
    for n in ("tvmr fbcr ptmr ewdr ewlr ewrr edsr lopr copr draw change_requested").split():
        print("vdp1", n, s["vdp1"][n])
    print("vdp2 flags", s["vdp2"]["vblank"], s["vdp2"]["hblank"], s["vdp2"]["odd"])
    print("sys:", {k: s[k] for k in TAIL_NAMES})
    print("io", s["io"].hex(), "inputs", s["inputs"].hex(), "prot", s["prot"].hex(),
          s["prot_on"], s["prot_counter"])


if __name__ == "__main__":
    main()
