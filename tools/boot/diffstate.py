#!/usr/bin/env python3
"""What differs between two boot runs' entry points (OUT dirs of the reach driver: entry.state,
dump/, lastw.bin): work RAM extents (with who wrote them last), the other memories' sizes of
difference, and the registers (CPUs, on-chip, SCU, SMPC, SYS fields).

    diffstate.py A B [--quiet]
"""
import argparse
import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(__file__))
import state  # noqa: E402
import writers  # noqa: E402


def extents(ne, gap=8):
    idx = np.flatnonzero(ne)
    if len(idx) == 0:
        return []
    cut = np.flatnonzero(np.diff(idx) >= gap)
    st = np.concatenate(([0], cut + 1))
    en = np.concatenate((cut, [len(idx) - 1]))
    return [(int(idx[s]), int(idx[e]) + 1) for s, e in zip(st, en)]


def flat(prefix, v, out):
    if isinstance(v, dict):
        for k, x in v.items():
            flat(f"{prefix}.{k}", x, out)
    elif isinstance(v, (list, tuple)):
        for i, x in enumerate(v):
            flat(f"{prefix}[{i}]", x, out)
    elif isinstance(v, (bytes, bytearray)):
        out[prefix] = bytes(v)
    else:
        out[prefix] = v


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("a")
    ap.add_argument("b")
    ap.add_argument("--gap", type=int, default=8)
    a = ap.parse_args()
    for name, base, lw0 in (("wram-h.bin", 0x06000000, 0), ("wram-l.bin", 0x00200000, 0x100000)):
        x = np.fromfile(f"{a.a}/dump/{name}", dtype=np.uint8)
        y = np.fromfile(f"{a.b}/dump/{name}", dtype=np.uint8)
        lw = np.fromfile(f"{a.a}/lastw.bin", dtype="<u4")[lw0:lw0 + len(x)]
        ex = extents(x != y, a.gap)
        print(f"== {name}: {int((x != y).sum()):#x} bytes differ in {len(ex)} extents")
        for s, e in ex:
            who = {writers.tag(int(v)) for v in np.unique(lw[s:e])}
            print(f"  {base + s:08x}-{base + e:08x} ({e - s:#x}) A={x[s:s + 8].tobytes().hex()} B={y[s:s + 8].tobytes().hex()} last written by {', '.join(sorted(who))[:100]}")
    for n in ("vdp1-vram.bin", "vdp2-vram.bin", "vdp2-cram.bin", "vdp2-regs.bin", "sound-ram.bin"):
        x = np.fromfile(f"{a.a}/dump/{n}", dtype=np.uint8)
        y = np.fromfile(f"{a.b}/dump/{n}", dtype=np.uint8)
        print(f"== {n}: {int((x != y).sum()):#x} bytes differ")
    sa, sb = state.load(f"{a.a}/entry.state"), state.load(f"{a.b}/entry.state")
    fa, fb = {}, {}
    for k in ("cpu", "sys"):
        flat(k, sa[k], fa)
        flat(k, sb[k], fb)
    skip = ("sys.wram", "sys.vdp1", "sys.vdp2", "sys.scsp", "sys.cache_data", "sys.backup")
    print("== registers and fields")
    for k in fa:
        if k.startswith(skip):
            continue
        if fa[k] != fb.get(k):
            va, vb = fa[k], fb.get(k)
            f = (lambda v: v.hex() if isinstance(v, bytes) else hex(v) if isinstance(v, int) and not isinstance(v, bool) else v)
            print(f"  {k}: {f(va)} -> {f(vb)}")
    for n in ("backup",):
        if sa["sys"][n] != sb["sys"][n]:
            d = np.frombuffer(sa["sys"][n], dtype=np.uint8) != np.frombuffer(sb["sys"][n], dtype=np.uint8)
            print(f"== backup RAM: {int(d.sum()):#x} bytes differ; extents {[(hex(s), hex(e)) for s, e in extents(d, 16)][:10]}")


if __name__ == "__main__":
    main()
