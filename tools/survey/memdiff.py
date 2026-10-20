"""Compares two memory dumps (the oracle's and the machine's): the ranges that differ, per memory.

    .venv/bin/python tools/survey/memdiff.py ORACLE_DUMP MACHINE_DUMP [--max N]
"""
import argparse
import pathlib

BASES = {"wram-l.bin": 0x00200000, "wram-h.bin": 0x06000000, "vdp1-vram.bin": 0x05C00000,
         "vdp2-vram.bin": 0x05E00000, "vdp2-cram.bin": 0x05F00000, "sound-ram.bin": 0x05A00000}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("a")
    ap.add_argument("b")
    ap.add_argument("--max", type=int, default=12)
    a = ap.parse_args()
    for name, base in BASES.items():
        x = (pathlib.Path(a.a) / name).read_bytes()
        y = (pathlib.Path(a.b) / name).read_bytes()
        runs, i, n = [], 0, min(len(x), len(y))
        while i < n:
            if x[i] != y[i]:
                j = i
                while j < n and (x[j] != y[j] or (j + 8 < n and x[j:j + 8] != y[j:j + 8])):
                    j += 1
                runs.append((i, j))
                i = j
            else:
                i += 1
        total = sum(e - s for s, e in runs)
        print(f"{name}: {total} bytes differ in {len(runs)} ranges")
        for s, e in runs[:a.max]:
            print(f"  {base + s:08x}-{base + e:08x} oracle {x[s:min(e, s + 16)].hex()}  machine {y[s:min(e, s + 16)].hex()}")


if __name__ == "__main__":
    main()
