#!/usr/bin/env python3
"""Who wrote each byte of work RAM last, from a last-writer map (4 bytes per RAM byte, little
endian: the writing instruction's address, bit 31 the CPU, 0xD... an SCU DMA, 0xE... an SH-2
DMA started by that instruction; made by a patched machine, see docs/HANDOFF.md).

    writers.py LASTW --ram h|l [--from A --to B] [--min 1]   extents by writer
    writers.py LASTW --ram h --by-pc                           bytes per writer address

The addresses are those of the writing code: BIOS ROM (0x0000....), BIOS RAM code (0x0600....).
"""
import argparse
import collections

import numpy as np


def tag(v):
    top = v >> 28
    cpu = (v >> 31) & 1
    if top in (0xD, 0xE):
        return ("scu-dma" if top == 0xD else "sh2-dma") + f" (from {v & 0x0FFFFFFF:08x})"
    return f"cpu{cpu} pc={v & 0x7FFFFFFF:08x}" if v else "never written"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("lastw")
    ap.add_argument("--ram", choices="hl", default="h")
    ap.add_argument("--from", dest="lo", type=lambda s: int(s, 0), default=0)
    ap.add_argument("--to", dest="hi", type=lambda s: int(s, 0), default=0x100000)
    ap.add_argument("--min", type=int, default=1)
    ap.add_argument("--by-pc", action="store_true")
    a = ap.parse_args()
    w = np.fromfile(a.lastw, dtype="<u4")
    w = w[:0x100000] if a.ram == "h" else w[0x100000:]
    base = 0x06000000 if a.ram == "h" else 0x00200000
    w = w[a.lo:a.hi]
    if a.by_pc:
        c = collections.Counter(w.tolist())
        for v, n in sorted(c.items()):
            print(f"{tag(v):40s} {n:#9x} bytes")
        return
    cut = np.flatnonzero(w[1:] != w[:-1]) + 1
    st = np.concatenate(([0], cut))
    en = np.concatenate((cut, [len(w)]))
    for s, e in zip(st, en):
        if e - s >= a.min:
            print(f"{base + a.lo + int(s):08x}-{base + a.lo + int(e):08x} ({int(e - s):#8x}) {tag(int(w[s]))}")


if __name__ == "__main__":
    main()
