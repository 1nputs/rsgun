#!/usr/bin/env python3
"""Splits a memory dump into extents by what is in them: the BIOS's SEGASEGA fill (aligned to 4),
zeros, 0xFF, or other (data). Prints the extents (merged under --min bytes of other kind).

    classify.py DUMP@BASE [--unit 4] [--min 16]
"""
import argparse

import numpy as np

SEGA = np.frombuffer(b"SEGA", dtype=np.uint8)


def kinds(mem, unit):
    n = len(mem) // unit * unit
    w = mem[:n].reshape(-1, unit)
    k = np.full(len(w), "D", dtype="<U1")  # data
    k[(w == 0).all(1)] = "0"
    k[(w == 0xFF).all(1)] = "F"
    k[(w == np.tile(SEGA, unit // 4)).all(1)] = "S" if unit % 4 == 0 else "D"
    return k


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("dump")
    ap.add_argument("--unit", type=int, default=4)
    ap.add_argument("--min", type=int, default=0x40, help="shortest extent shown on its own")
    a = ap.parse_args()
    path, base = a.dump.rsplit("@", 1)
    base = int(base, 0)
    mem = np.frombuffer(open(path, "rb").read(), dtype=np.uint8)
    k = kinds(mem, a.unit)
    cut = np.flatnonzero(k[1:] != k[:-1]) + 1
    st = np.concatenate(([0], cut))
    en = np.concatenate((cut, [len(k)]))
    out = []
    for s, e in zip(st, en):
        out.append([int(s) * a.unit, int(e) * a.unit, str(k[s])])
    # fold short extents into their neighbours' "data"
    merged = []
    for s, e, c in out:
        if e - s < a.min and c != "D" and merged:
            c = "D"
        if merged and merged[-1][2] == c:
            merged[-1][1] = e
        else:
            merged.append([s, e, c])
    names = {"S": "SEGA fill", "0": "zeros", "F": "0xFF", "D": "data"}
    for s, e, c in merged:
        print(f"{base + s:08x}-{base + e:08x} ({e - s:#8x}) {names[c]}")


if __name__ == "__main__":
    main()
