#!/usr/bin/env python3
"""Compares two save states (e.g. two runs' ends): memories by extents, registers by name.

    cmpstates.py A B [--gap 8] [--max 12]
"""
import argparse
import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(__file__))
import diffstate  # noqa: E402
import state  # noqa: E402


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("a")
    ap.add_argument("b")
    ap.add_argument("--gap", type=int, default=8)
    ap.add_argument("--max", type=int, default=12)
    a = ap.parse_args()
    sa, sb = state.load(a.a)["sys"], state.load(b := a.b)["sys"]
    same = True
    for name in ("wram_l", "wram_h", "backup", "vdp1.vram", "vdp2.vram", "vdp2.cram", "vdp2.regs", "scsp.ram", "scsp.regs"):
        def get(s):
            for part in name.split("."):
                s = s[part]
            return s
        x = np.frombuffer(get(sa), dtype=np.uint8)
        y = np.frombuffer(get(sb), dtype=np.uint8)
        ex = diffstate.extents(x != y, a.gap)
        if ex:
            same = False
            print(f"{name}: {int((x != y).sum()):#x} bytes differ in {len(ex)} extents: " + ", ".join(f"{s:#x}-{e:#x}" for s, e in ex[:a.max]))
    fa, fb = {}, {}
    for k, st in (("cpu", state.load(a.a)["cpu"]), ("sys", sa)):
        diffstate.flat(k, st, fa)
    for k, st in (("cpu", state.load(b)["cpu"]), ("sys", sb)):
        diffstate.flat(k, st, fb)
    skip = ("sys.wram", "sys.vdp1", "sys.vdp2", "sys.scsp", "sys.cache_data", "sys.backup")
    n = 0
    for k in fa:
        if k.startswith(skip) or fa[k] == fb.get(k):
            continue
        same = False
        n += 1
        if n <= a.max:
            print(f"{k}: {fa[k]!r} -> {fb[k]!r}")
    if same:
        print("identical")


if __name__ == "__main__":
    main()
