#!/usr/bin/env python3
"""Finds where a memory dump's bytes come from, by content: runs of it equal to runs of the cart
image and the BIOS ROM (nothing of either is written out: offsets and lengths only).

    sources.py DUMP@BASE [DUMP@BASE ...]  (e.g. wram-h.bin@0x06000000)
    options:     [--cart build/rom/rsgun-cart.bin] [--bios build/rom/bios-epr20091.bin]
                    [--min 32] [--list]

Prints, for each run of at least --min bytes found in a source, `ram_range <- source:offset`, then
the extents of non-zero bytes no source explains. Matching is at 4-byte steps (the loaders copy
words); a run may contain bytes the game has changed since (not reported here; see compare.py).
"""
import argparse
import sys

import numpy as np


def keys(buf):
    """u64 at each 4-byte offset (big-endian words, as the SH-2 sees them)."""
    n = (len(buf) - 8) // 4 + 1
    a = np.frombuffer(buf, dtype=np.uint8)
    idx = np.arange(n, dtype=np.int64) * 4
    k = np.zeros(n, dtype=np.uint64)
    for j in range(8):
        k = (k << np.uint64(8)) | a[idx + j].astype(np.uint64)
    return k


class Index:
    def __init__(self, sources):
        self.names, self.bufs = zip(*sources)
        self.keys = [keys(b) for b in self.bufs]
        self.order = [np.argsort(k, kind="stable") for k in self.keys]
        self.sorted = [k[o] for k, o in zip(self.keys, self.order)]

    def candidates(self, key, limit=64):
        out = []
        for s, (sk, so) in enumerate(zip(self.sorted, self.order)):
            key = np.uint64(key)
            lo = np.searchsorted(sk, key, "left")
            hi = np.searchsorted(sk, key, "right")
            if hi - lo > limit:  # a pattern that repeats: no source of its own
                continue
            for i in so[lo:hi]:
                out.append((s, int(i) * 4))
        return out


def common(a, ai, b, bi, limit):
    """Length of the equal run of a[ai:] and b[bi:] (up to limit), compared in growing chunks."""
    n = min(limit, len(a) - ai, len(b) - bi)
    done, chunk = 0, 256
    while done < n:
        c = min(chunk, n - done)
        x = np.frombuffer(a, dtype=np.uint8, count=c, offset=ai + done)
        y = np.frombuffer(b, dtype=np.uint8, count=c, offset=bi + done)
        ne = np.flatnonzero(x != y)
        if len(ne):
            return done + int(ne[0])
        done += c
        chunk *= 4
    return n


def runs(dump, idx, minlen=32, step=4):
    """Greedy cover of `dump` by runs found in the sources: [(start, length, source, offset)]."""
    out = []
    i = 0
    n = len(dump)
    k = keys(dump)
    while i + 8 <= n:
        best = (0, None, 0)
        w = dump[i:i + 8]
        if w.count(w[0]) != 8:  # an all-equal window explains nothing
            for s, off in idx.candidates(int(k[i // 4])):
                l = common(dump, i, idx.bufs[s], off, n - i)
                if l > best[0]:
                    best = (l, s, off)
        if best[0] >= minlen:
            out.append((i, best[0], best[1], best[2]))
            i += best[0] - best[0] % step
            if best[0] % step:
                i += step
        else:
            i += step
    return out


def unexplained(dump, found, floor=0):
    """Extents of non-zero bytes outside the found runs: [(start, end)] (gaps merged under 16)."""
    covered = np.zeros(len(dump), dtype=bool)
    for st, l, _, _ in found:
        covered[st:st + l] = True
    nz = np.frombuffer(dump, dtype=np.uint8) != 0
    left = nz & ~covered
    idx = np.flatnonzero(left)
    if len(idx) == 0:
        return []
    cut = np.flatnonzero(np.diff(idx) >= 16)
    starts = np.concatenate(([0], cut + 1))
    ends = np.concatenate((cut, [len(idx) - 1]))
    ext = [[int(idx[s]), int(idx[e]) + 1] for s, e in zip(starts, ends)]
    return ext


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("dumps", nargs="+", help="FILE@BASE")
    ap.add_argument("--cart", default="build/rom/rsgun-cart.bin")
    ap.add_argument("--bios", default="build/rom/bios-epr20091.bin")
    ap.add_argument("--min", type=int, default=32)
    ap.add_argument("--from", dest="lo", type=lambda s: int(s, 0), default=0)
    ap.add_argument("--to", dest="hi", type=lambda s: int(s, 0), default=None)
    a = ap.parse_args()
    idx = Index([("cart", open(a.cart, "rb").read()), ("bios", open(a.bios, "rb").read())])
    for spec in a.dumps:
        path, base = spec.rsplit("@", 1)
        base = int(base, 0)
        dump = open(path, "rb").read()[a.lo:a.hi]
        print(f"== {path} at {base:#x}")
        found = runs(dump, idx, a.min)
        for st, l, s, off in found:
            print(f"{base + a.lo + st:08x}-{base + a.lo + st + l:08x} ({l:#x}) <- {idx.names[s]}:{off:#x}")
        tot = sum(f[1] for f in found)
        print(f"-- {tot:#x} bytes explained in {len(found)} runs")
        for lo, hi in unexplained(dump, found):
            print(f"unexplained {base + a.lo + lo:08x}-{base + a.lo + hi:08x} ({hi - lo:#x})")


if __name__ == "__main__":
    main()
