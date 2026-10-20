"""Which hardware registers the traced code touched: every data access outside work RAM and ROM,
grouped by device and address, with read/write counts and the values seen.

    .venv/bin/python tools/survey/iomap.py TRACE_DIR...   (directories holding m.trace / s.trace)
"""
import collections
import pathlib
import struct
import sys

DEVICES = [  # (first, last, name) on the 27-bit bus, plus the on-chip area
    (0x00100000, 0x0017FFFF, "SMPC"), (0x00180000, 0x001FFFFF, "backup/EEPROM"),
    (0x00400000, 0x0040FFFF, "ST-V I/O"), (0x01000000, 0x017FFFFF, "MINIT (slave FRT)"),
    (0x01800000, 0x01FFFFFF, "SINIT (master FRT)"), (0x04FFFFF0, 0x04FFFFFF, "cart protection"),
    (0x05A00000, 0x05AFFFFF, "SCSP RAM"), (0x05B00000, 0x05BFFFFF, "SCSP regs"),
    (0x05C00000, 0x05C7FFFF, "VDP1 VRAM"), (0x05C80000, 0x05CFFFFF, "VDP1 framebuffer"),
    (0x05D00000, 0x05D7FFFF, "VDP1 regs"), (0x05E00000, 0x05EFFFFF, "VDP2 VRAM"),
    (0x05F00000, 0x05F7FFFF, "VDP2 CRAM"), (0x05F80000, 0x05FBFFFF, "VDP2 regs"),
    (0x05FE0000, 0x05FEFFFF, "SCU regs"),
]
BULK = {"SCSP RAM", "VDP1 VRAM", "VDP1 framebuffer", "VDP2 VRAM", "VDP2 CRAM"}


def device(a):
    if a >= 0xFFFFFE00:
        return "on-chip"
    region = a >> 29
    if region in (2, 3):
        return "cache purge/array"
    a &= 0x07FFFFFF
    if 0x00200000 <= a < 0x00300000 or a >= 0x06000000 or a < 0x00100000 or 0x02000000 <= a < 0x04FFFFF0:
        return None
    for lo, hi, name in DEVICES:
        if lo <= a <= hi:
            return name
    return "unmapped"


def main():
    acc = collections.defaultdict(lambda: [0, 0, set(), set()])  # reads, writes, sizes, values
    bulk = collections.Counter()
    for d in sys.argv[1:]:
        for f in ("m.trace", "s.trace"):
            p = pathlib.Path(d) / f
            if not p.exists():
                continue
            data = p.read_bytes()
            cpu = f[0]
            for i in range(0, len(data), 12):
                tag, size, _, a, v = struct.unpack(">BBHII", data[i:i + 12])
                if tag not in (ord("R"), ord("W")):
                    continue
                dev = device(a)
                if dev is None:
                    continue
                if dev in BULK:
                    bulk[(dev, chr(tag), cpu)] += 1
                    continue
                key = (dev, (a & 0x07FFFFFF) if a < 0xFFFFFE00 else a, cpu)
                e = acc[key]
                e[0 if tag == ord("R") else 1] += 1
                e[2].add(size)
                if len(e[3]) < 6:
                    e[3].add(v)
    for (dev, a, cpu), (r, w, sizes, vals) in sorted(acc.items()):
        vs = " ".join(f"{v:x}" for v in sorted(vals))
        print(f"{dev:20s} {a:08x} {cpu} r{r:<7d} w{w:<7d} sizes {sorted(sizes)}  values {vs}")
    print()
    for (dev, t, cpu), n in sorted(bulk.items()):
        print(f"{dev:20s} {t} {cpu} {n}")


if __name__ == "__main__":
    main()
