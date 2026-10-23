#!/usr/bin/env python3
"""Lists the bytes of work RAM the BIOS computed (last written by neither its fill, nor a bulk
copy from ROM, nor the game's load), with their values and writers, from a reach run's dump/ and
lastw.bin. A bulk writer is one that wrote at least --bulk bytes in all.

    variables.py OUT --ram h [--bulk 64] [--from A --to B]
"""
import argparse
import collections
import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(__file__))
import writers  # noqa: E402


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("out")
    ap.add_argument("--ram", choices="hl", default="h")
    ap.add_argument("--bulk", type=int, default=0x200)
    ap.add_argument("--from", dest="lo", type=lambda s: int(s, 0), default=0)
    ap.add_argument("--to", dest="hi", type=lambda s: int(s, 0), default=0x100000)
    a = ap.parse_args()
    mem = np.fromfile(f"{a.out}/dump/wram-{a.ram}.bin", dtype=np.uint8)
    lw = np.fromfile(f"{a.out}/lastw.bin", dtype="<u4")
    lw = lw[:0x100000] if a.ram == "h" else lw[0x100000:]
    base = 0x06000000 if a.ram == "h" else 0x00200000
    count = collections.Counter(lw.tolist())
    bulk = {v for v, n in count.items() if n >= a.bulk}
    print("bulk writers:", ", ".join(f"{writers.tag(v)} {count[v]:#x}" for v in sorted(bulk)))
    comp = ~np.isin(lw, list(bulk))
    comp[:a.lo] = False
    comp[a.hi:] = False
    print(f"computed bytes: {int(comp.sum()):#x}")
    idx = np.flatnonzero(comp)
    if not len(idx):
        return
    # extents of equal writer
    cur = None
    for i in idx:
        w = int(lw[i])
        if cur and cur[2] == w and i == cur[1]:
            cur[1] = i + 1
        else:
            if cur:
                emit(cur, mem, base)
            cur = [int(i), int(i) + 1, w]
    emit(cur, mem, base)


def emit(c, mem, base):
    s, e, w = c
    print(f"{base + s:08x}-{base + e:08x} {writers.tag(w):22s} {mem[s:min(e, s + 16)].tobytes().hex(' ', 2)}")


if __name__ == "__main__":
    main()
