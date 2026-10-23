#!/usr/bin/env python3
"""The simple part of a native boot: builds work RAM-H as the handover finds it from the images
alone (the BIOS's fill, the cart's game image, the BIOS ROM's copies, the EEPROM) and lists what
that leaves unexplained against a reach run's dump (the bytes the BIOS computes).

    recipe.py OUT [--rom build/rom] [--eeprom FILE] [--writers]

OUT: a reach run's directory (dump/wram-h.bin, lastw.bin). Offsets and sizes are the recipe:
  fill       all of work RAM-H and -L: "SEGA" repeated
  game       cart[0x280000:+0x38000]        -> 0x06004000  (the cart header's boot table, area 1)
  bios area  bios[0x000600:+0x000800]       -> 0x06000000  (vectors, service table); zeros to 0x06000c00
  bios code  bios[0x030000:+0x002400]       -> 0x06000c00  (the BIOS's code in RAM)
  bios data  bios[0x066000:+0x00a000]       -> 0x060f0000
  zeros      0x060fa000..0x060fb600, 0x06003000..0x06004000
  eeprom     the 128-byte image             -> 0x06000780
"""
import argparse
import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(__file__))
import diffstate  # noqa: E402
import writers  # noqa: E402


def template(rom, eeprom):
    cart = open(f"{rom}/rsgun-cart.bin", "rb").read()
    bios = open(f"{rom}/bios-epr20091.bin", "rb").read()
    ram = bytearray(b"SEGA" * (0x100000 // 4))

    def put(addr, data):
        ram[addr - 0x06000000:addr - 0x06000000 + len(data)] = data

    put(0x06000000, bios[0x600:0x600 + 0x800])
    put(0x06000800, bytes(0x400))
    put(0x06000C00, bios[0x30000:0x30000 + 0x2400])
    put(0x06003000, bytes(0x1000))
    put(0x06004000, cart[0x280000:0x280000 + 0x38000])
    put(0x060F0000, bios[0x66000:0x66000 + 0xA000])
    put(0x060FA000, bytes(0x1600))
    put(0x06000780, eeprom)
    return np.frombuffer(bytes(ram), dtype=np.uint8)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("out")
    ap.add_argument("--rom", default="build/rom")
    ap.add_argument("--eeprom")
    ap.add_argument("--writers", action="store_true")
    ap.add_argument("--gap", type=int, default=16)
    a = ap.parse_args()
    ee = open(a.eeprom or f"{a.rom}/eeprom.bin", "rb").read()
    mine = template(a.rom, ee)
    real = np.fromfile(f"{a.out}/dump/wram-h.bin", dtype=np.uint8)
    lw = np.fromfile(f"{a.out}/lastw.bin", dtype="<u4")[:0x100000]
    ne = mine != real
    ex = diffstate.extents(ne, a.gap)
    print(f"template leaves {int(ne.sum()):#x} of 0x100000 bytes in {len(ex)} extents")
    for s, e in ex:
        who = sorted({writers.tag(int(v)).replace("cpu0 ", "") for v in np.unique(lw[s:e])})
        print(f"  {0x06000000 + s:08x}-{0x06000000 + e:08x} ({e - s:#x}) real={real[s:s + 8].tobytes().hex()} {', '.join(who)[:70] if a.writers else ''}")


if __name__ == "__main__":
    main()
