"""Summarise or print an oracle trace (tools/patches/mednafen/oracle.inc, version 3).

    .venv/bin/python tools/trace/show.py TRACE [--from N] [--count N]
"""
import argparse
import collections
import struct


def records(path):
    data = open(path, "rb").read()
    for i in range(0, len(data) - 11, 12):
        tag, size, _, a, v = struct.unpack(">BBHII", data[i:i + 12])
        yield chr(tag), size, a, v


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("trace")
    ap.add_argument("--from", dest="start", type=int, default=0)
    ap.add_argument("--count", type=int, default=0)
    a = ap.parse_args()
    if not a.count:
        c = collections.Counter((t, s) for t, s, _, _ in records(a.trace))
        print({f"{t}{s}": n for (t, s), n in sorted(c.items())})
        return
    for k, (t, s, ad, v) in enumerate(records(a.trace)):
        if k < a.start:
            continue
        if k >= a.start + a.count:
            break
        if t == "P":
            print(f"{k:8d} P {ad:08x}  op {s:02x} {v:04x}")
        elif t in "RW":
            print(f"{k:8d}   {t}{s} [{ad:08x}] {v:0{2 * s}x}")
        else:
            print(f"{k:8d} {t} size={s} a={ad:08x} v={v:08x}")


if __name__ == "__main__":
    main()
