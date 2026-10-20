"""Find which parts of a memory dump are copies of the cartridge: the load map.

    .venv/bin/python tools/survey/loadmap.py DUMP_DIR [--min 256]

For work RAM-H ($06000000) and RAM-L ($00200000) in an oracle dump, probes the memory every --min/2
bytes and, wherever 32 bytes match somewhere in the cart image (build/rom/rsgun-cart.bin, at
$02000000), extends the match both ways and reports the run: memory range <- cart range. Every run of
--min bytes or more contains a probe, so none is missed; shorter ones may be.
"""
import argparse
import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[2]
CART_BASE = 0x02000000


def runs(mem: bytes, cart: bytes, base: int, minlen: int):
    out, a, n = [], 0, len(mem)
    step = max(32, minlen // 2 - 32)
    while a < n - 32:
        key = mem[a:a + 32]
        if key.count(key[0]) == 32:                  # fill: never a meaningful match
            a += step
            continue
        c = cart.find(key)
        if c < 0:
            a += step
            continue
        s, cs = a, c                                 # extend back, then forward
        while s > 0 and cs > 0 and mem[s - 1] == cart[cs - 1]:
            s -= 1
            cs -= 1
        e, ce = a + 32, c + 32
        while e < n and ce < len(cart) and mem[e] == cart[ce]:
            e += 1
            ce += 1
        if e - s >= minlen:
            out.append((base + s, base + e, CART_BASE + cs))
        a = e
    return out


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("dump")
    ap.add_argument("--min", type=int, default=256)
    a = ap.parse_args()
    cart = (ROOT / "build/rom/rsgun-cart.bin").read_bytes()
    d = pathlib.Path(a.dump)
    for name, base in (("wram-h.bin", 0x06000000), ("wram-l.bin", 0x00200000)):
        mem = (d / name).read_bytes()
        found = runs(mem, cart, base, a.min)
        total = sum(e - s for s, e, _ in found)
        print(f"{name}: {total:#x} bytes from the cart in {len(found)} runs")
        for s, e, c in found:
            print(f"  {s:08x}-{e:08x} ({e - s:#8x}) <- cart {c:08x}-{c + e - s:08x}")


if __name__ == "__main__":
    main()
