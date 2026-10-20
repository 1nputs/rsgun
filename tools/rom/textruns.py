"""Find runs of text (ASCII and Shift-JIS) in an image; write each run to OUTDIR/<offset>.txt."""
import sys, os
d = open(sys.argv[1], "rb").read()
out = sys.argv[2]; os.makedirs(out, exist_ok=True)
W = 256
def texty(b):
    n, i, good = len(b), 0, 0
    while i < n:
        c = b[i]
        if 32 <= c < 127 or c in (9, 10, 13):
            good += 1; i += 1
        elif (0x81 <= c <= 0x9F or 0xE0 <= c <= 0xFC) and i + 1 < n and 0x40 <= b[i + 1] <= 0xFC:
            good += 2; i += 2
        else:
            i += 1
    return good / n
runs, cur = [], None
for o in range(0, len(d), W):
    t = texty(d[o:o + W]) > 0.9 and d[o:o + W].count(b"\n") >= 2
    if t and cur is None: cur = o
    if not t and cur is not None: runs.append((cur, o)); cur = None
if cur is not None: runs.append((cur, len(d)))
total = 0
for a, b in runs:
    if b - a < 1024: continue
    blob = d[a:b]
    txt = blob.decode("cp932", errors="replace")
    open(os.path.join(out, f"{a:07x}.txt"), "w").write(txt)
    lines = txt.count("\n")
    kind = "C" if ("->" in txt or "){" in txt or "static" in txt) else "other"
    total += b - a
    print(f"{a:07x}-{b:07x} {b - a:7d} bytes {lines:5d} lines {kind}")
print("total", total)
