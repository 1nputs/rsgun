"""Who calls a function and what it calls, from the evidence (rsgun --evidence): for the rewrite.

    python3 tools/port/calls.py ADDR [--evidence out/evidence/play1.txt,out/evidence/attract.txt]

Each caller is given as the function its call site is in (the nearest function entry the port has
at or before the site: build/port/generated.rs), with the site and how often the evidence saw it.
"""
import argparse
import bisect
import collections
import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[2]


def entries():
    text = (ROOT / "build/port/generated.rs").read_text()
    by_fp = collections.defaultdict(list)
    for m in re.finditer(r"^pub fn f_([0-9a-f]{8})(?:_([0-9a-f]{16}))?\(", text, re.M):
        by_fp[m.group(2) or "0" * 16].append(int(m.group(1), 16))
    return {fp: sorted(v) for fp, v in by_fp.items()}


def owner(funcs, fp, site):
    lst = funcs.get(fp) or funcs.get("0" * 16, [])
    i = bisect.bisect_right(lst, site) - 1
    if i < 0:
        return "?"
    name = f"f_{lst[i]:08x}"
    return name if fp == "0" * 16 else f"{name}_{fp}"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("addr")
    ap.add_argument("--evidence", default="out/evidence/play1.txt,out/evidence/attract.txt")
    args = ap.parse_args()
    target = int(args.addr, 16)
    funcs = entries()
    callers = collections.Counter()
    callees = collections.Counter()
    lst = sorted(set().union(*funcs.values()))
    i = bisect.bisect_right(lst, target)
    end = lst[i] if i < len(lst) else target + 0x10000
    for path in args.evidence.split(","):
        for line in (ROOT / path).read_text().splitlines():
            p = line.split()
            if not p or p[0] != "t" or p[3] not in ("c", "j"):
                continue
            fp, kind, site, to, n = p[2], p[3], int(p[4], 16), int(p[5], 16), int(p[6])
            if to == target:
                callers[(owner(funcs, fp, site), site, kind)] += n
            if target <= site < end and kind == "c":
                callees[to] += n
    print(f"callers of {target:08x}:")
    for (f, site, kind), n in sorted(callers.items()):
        print(f"  {f:34} site {site:08x} {'call' if kind == 'c' else 'jump'} x{n}")
    print(f"calls from {target:08x}-{end:08x}:")
    for to, n in sorted(callees.items()):
        print(f"  f_{to:08x} x{n}")


if __name__ == "__main__":
    main()
