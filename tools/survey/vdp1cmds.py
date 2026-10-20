"""Walks VDP1 command lists in draw dumps (DIR/draw/vdp1-vram.bin) as VDP1 does and counts the commands
and draw modes they use.

    .venv/bin/python tools/survey/vdp1cmds.py DUMP_DIR... [--list]
"""
import collections
import pathlib
import struct
import sys

NAMES = ["normal", "scaled", "distorted", "distorted3", "polygon", "polyline", "line", "polyline7",
         "userclip", "sysclip", "local", "userclipB", "C", "D", "E", "F"]


def walk(vram):
    addr, ret, n = 0, None, 0
    while n < 20000:
        n += 1
        w = struct.unpack(">16H", vram[addr:addr + 32])
        ctrl = w[0]
        if ctrl & 0x8000:
            return
        jp = (ctrl >> 12) & 7
        if jp < 4:
            yield addr, w
        link = (w[1] & 0xFFFC) << 3
        k = jp & 3
        if k == 0:
            addr += 32
        elif k == 1:
            addr = link
        elif k == 2:
            ret, addr = addr + 32, link
        else:
            addr, ret = (ret if ret is not None else addr + 32), None
        addr &= 0x7FFE0


def main():
    listing = "--list" in sys.argv
    kinds, modes = collections.Counter(), collections.Counter()
    for d in [a for a in sys.argv[1:] if not a.startswith("--")]:
        vram = (pathlib.Path(d) / "draw/vdp1-vram.bin").read_bytes()
        for addr, w in walk(vram):
            c = w[0] & 0xF
            kinds[NAMES[c]] += 1
            if c < 8:
                pmod = w[2]
                cm = (pmod >> 3) & 7
                cc = pmod & 7
                flags = [n for b, n in ((15, "MSB"), (12, "HSS"), (11, "PCLP"), (10, "CLIP"), (8, "MESH"), (7, "ECD"), (6, "SPD")) if pmod & (1 << b)]
                modes[(NAMES[c], f"color{cm}", ["replace", "shadow", "halflum", "halftrans", "gouraud", "?5", "gouraud+halflum", "gouraud+halftrans"][cc], " ".join(flags))] += 1
                if listing:
                    print(f"{addr:05x} {NAMES[c]:10s} pmod {pmod:04x} colr {w[3]:04x} srca {w[4]:04x} size {w[5]:04x} A({w[6]:04x},{w[7]:04x}) B({w[8]:04x},{w[9]:04x}) C({w[10]:04x},{w[11]:04x}) D({w[12]:04x},{w[13]:04x}) dir {(w[0] >> 4) & 3} zp {(w[0] >> 8) & 15}")
    print("commands:", dict(kinds))
    for k, n in modes.most_common():
        print(f"  {n:6d}  {k}")


if __name__ == "__main__":
    main()
