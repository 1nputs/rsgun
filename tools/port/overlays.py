"""Where the overlay window's code came from: for each dump, the cart file offset its first bytes
match, and how far the match runs.

    python3 tools/port/overlays.py [--cart build/rom/rsgun-cart.bin] DUMP_DIR...

The window is work RAM-H 0x06080000-0x060A5000 (crates/rsgun's overlay()).
"""
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]


def main():
    args = sys.argv[1:]
    cart_path = ROOT / "build/rom/rsgun-cart.bin"
    if args[:1] == ["--cart"]:
        cart_path, args = pathlib.Path(args[1]), args[2:]
    cart = cart_path.read_bytes()
    for d in args:
        w = (pathlib.Path(d) / "wram-h.bin").read_bytes()[0x80000:0xA5000]
        head = w[:64]
        at = cart.find(head)
        if at < 0:
            print(f"{d}: window start not found in the cart")
            continue
        n = 0
        while n < len(w) and cart[at + n] == w[n]:
            n += 1
        print(f"{d}: cart +{at:#08x} (address {0x02000000 + at:#010x}), {n:#x} bytes match from 0x06080000")


main()
