#!/usr/bin/env python3
"""Writes a copy of a save state with work RAM ranges filled (to test that they are leftovers).

    stateedit.py IN OUT --fill h:0x060f0000:0x060fb600:00 [--fill l:...:SEGA] ...
    stateedit.py IN OUT --dump DIR --fillmem vdp1-vram:0:0x80000:00 ...

--fillmem edits another memory of the machine (vdp1-vram, vdp2-vram, vdp2-cram, sound-ram), by
offset; DIR is a dump of that very state (it is how the memory is found in the file).

RAM is h (work RAM-H, 0x06000000) or l (work RAM-L, 0x00200000); the value is a byte in hex, or
SEGA (the BIOS's own fill, S at addresses divisible by 4).
"""
import argparse
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
import state  # noqa: E402


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("src")
    ap.add_argument("dst")
    ap.add_argument("--fill", action="append", default=[])
    ap.add_argument("--fillmem", action="append", default=[])
    ap.add_argument("--dump")
    a = ap.parse_args()
    d = bytearray(open(a.src, "rb").read())
    off = state.offsets(a.src)
    for f in a.fill:
        ram, lo, hi, val = f.split(":")
        base, at = (0x06000000, off["wram_h"]) if ram == "h" else (0x00200000, off["wram_l"])
        lo, hi = int(lo, 0), int(hi, 0)
        for p in range(lo, hi):
            d[at + p - base] = b"SEGA"[p & 3] if val == "SEGA" else int(val, 16)
    for f in a.fillmem:
        name, lo, hi, val = f.split(":")
        mem = open(f"{a.dump}/{name}.bin", "rb").read()
        at = bytes(d).find(mem)
        assert at >= 0 and bytes(d).find(mem, at + 1) < 0, f"{name}: not found once in the state"
        lo, hi = int(lo, 0), int(hi, 0)
        d[at + lo:at + hi] = bytes([int(val, 16)]) * (hi - lo)
    open(a.dst, "wb").write(d)


if __name__ == "__main__":
    main()
