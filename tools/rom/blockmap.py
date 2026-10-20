"""Classify a binary in fixed blocks: fill bytes, ASCII-heavy, or other; print runs."""
import sys
data = open(sys.argv[1], "rb").read()
start, end, bs = int(sys.argv[2], 16), int(sys.argv[3], 16), int(sys.argv[4], 16)
def kind(b):
    if b.count(0xFF) == len(b): return "FF"
    if b.count(0) == len(b): return "00"
    asc = sum(32 <= c < 127 or c in (9, 10, 13) for c in b) / len(b)
    return "text" if asc > 0.85 else "bin"
prev, run = None, start
for o in range(start, end, bs):
    k = kind(data[o:o + bs])
    if k != prev:
        if prev: print(f"{run:07x}-{o:07x} {prev}")
        prev, run = k, o
print(f"{run:07x}-{end:07x} {prev}")
