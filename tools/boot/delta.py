#!/usr/bin/env python3
"""Compares a memory dump with a source image at a fixed offset (RAM address -> source offset),
by block: how much of each block equals the source, and the extents where it does not (what the
game wrote since the load: variables, the cleared bss).

    delta.py DUMP@BASE SOURCE --map RAM_START:RAM_END:SOURCE_OFFSET [--block 0x1000] [--gaps]

e.g. delta.py wram-h.bin@0x06000000 build/rom/rsgun-cart.bin --map 0x06004000:0x060f0000:0x280000
"""
import argparse

import numpy as np


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("dump")
    ap.add_argument("source")
    ap.add_argument("--map", required=True)
    ap.add_argument("--block", type=lambda s: int(s, 0), default=0x1000)
    ap.add_argument("--gaps", action="store_true", help="list each mismatch extent")
    ap.add_argument("--min-gap", type=int, default=16, help="merge mismatches closer than this")
    a = ap.parse_args()
    path, base = a.dump.rsplit("@", 1)
    base = int(base, 0)
    lo, hi, off = (int(x, 0) for x in a.map.split(":"))
    mem = np.frombuffer(open(path, "rb").read(), dtype=np.uint8)
    src = np.frombuffer(open(a.source, "rb").read(), dtype=np.uint8)
    m = mem[lo - base:hi - base]
    s = src[off:off + (hi - lo)]
    n = min(len(m), len(s))
    ne = m[:n] != s[:n]
    for b in range(0, n, a.block):
        c = ne[b:b + a.block]
        print(f"{lo + b:08x} {100 - 100 * c.sum() / len(c):6.2f}% equal")
    if a.gaps:
        idx = np.flatnonzero(ne)
        if len(idx):
            cut = np.flatnonzero(np.diff(idx) >= a.min_gap)
            st = np.concatenate(([0], cut + 1))
            en = np.concatenate((cut, [len(idx) - 1]))
            for x, y in zip(st, en):
                print(f"differs {lo + int(idx[x]):08x}-{lo + int(idx[y]) + 1:08x} ({int(idx[y]) + 1 - int(idx[x]):#x})")


if __name__ == "__main__":
    main()
