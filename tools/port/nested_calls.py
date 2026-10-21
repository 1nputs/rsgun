"""Checks each `nested!` call in crates/rsgun's game/ against the SH-2 code: the rewrite's SH-2
function (or, for a helper, the rewrites that call it) must call that entry: its address is among
the function's literals, the literals its PC-relative loads reach, or its bsr targets (the code
from its entry to the next function's, in work RAM-H's dump; the game's code calls through a
literal, SGL's asm by bsr too). A nested call the SH-2 function does not make (the callee's
code inline in it) gives the callee a frame the SH-2 code never had: make it a plain call marked
`// inline`. Exit status 1 if any are listed.

    python3 tools/port/nested_calls.py [IMAGE]   # default: out/evidence/dumps/f003000/wram-h.bin
"""
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
GAME = ROOT / "crates/rsgun/src/game"


def literals_by_function(image, entries):
    """{function entry: {32-bit words in its range}} for the main program (work RAM-H). A range
    with no return in it (no rts) runs on into the next: that entry is another way into the same
    code (SGL's slRotXSC is slRotX past its call for the sine)."""
    entries = sorted(e for e in entries if 0x06000000 <= e < 0x06080000)
    out = {}

    def returns(lo, hi):
        return any(image[i:i + 2] == b"\x00\x0b" for i in range(lo - 0x06000000, hi - 0x06000000, 2))

    for k, e in enumerate(entries):
        j = k + 1
        while j < len(entries) and not returns(entries[j - 1] if j > k + 1 else e, entries[j]):
            j += 1
        end = entries[j] if j < len(entries) else e + 0x1000
        a, b = e - 0x06000000, min(end, e + 0x4000) - 0x06000000
        out[e] = {int.from_bytes(image[i:i + 4], "big") for i in range(a & ~3, b, 4)}
        # and what it calls by bsr (PC-relative: no literal), and the literals its mov.l @(disp,PC)
        # loads wherever they are (SGL's pools can sit past the next function's entry)
        for i in range(a, b, 2):
            op = int.from_bytes(image[i:i + 2], "big")
            if op & 0xF000 == 0xB000:
                disp = op & 0xFFF
                disp -= 0x1000 if disp & 0x800 else 0
                out[e].add(0x06000000 + i + 4 + 2 * disp)
            elif op & 0xF000 == 0xD000:
                at = ((i + 4) & ~3) + 4 * (op & 0xFF)
                if at + 4 <= len(image):
                    out[e].add(int.from_bytes(image[at:at + 4], "big"))
    return out


def main() -> int:
    image = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else ROOT / "out/evidence/dumps/f003000/wram-h.bin").read_bytes()
    table = (GAME / "mod.rs").read_text()
    registered = {}
    for m in re.finditer(r"0x([0-9a-f]{4})_([0-9a-f]{4}) => (\w+)::(\w+)\[", table):
        registered.setdefault((m.group(3), m.group(4)), int(m.group(1) + m.group(2), 16))
    generated = (ROOT / "build/port/generated.rs").read_text()
    entries = {int(m.group(1), 16) for m in re.finditer(r"^pub fn f_([0-9a-f]{8})\(", generated, re.M)}
    entries |= set(registered.values())
    calls = literals_by_function(image, entries)
    # each Rust function's nested! callees, and the functions it calls plainly (for helpers)
    fn_re = re.compile(r"^(?:pub )?fn (\w+)", re.M)
    found = 0
    for path in sorted(GAME.glob("*.rs")):
        if path.name == "mod.rs":
            continue
        module = path.stem
        text = path.read_text()
        starts = [(m.start(), m.group(1)) for m in fn_re.finditer(text)]
        for k, (start, name) in enumerate(starts):
            body = text[start:starts[k + 1][0] if k + 1 < len(starts) else len(text)]
            owners = []
            if (module, name) in registered:
                owners = [registered[(module, name)]]
            else:
                # a helper: the registered functions of this file that call it
                for s2, n2 in starts:
                    if (module, n2) in registered and re.search(rf"\b{name}\(cpu", text[s2:]) and n2 != name:
                        b2 = text[s2:]
                        nxt = fn_re.search(b2, 1)
                        if re.search(rf"\b{name}\(cpu", b2[:nxt.start() if nxt else len(b2)]):
                            owners.append(registered[(module, n2)])
            for m in re.finditer(r"nested!\(cpu,\s*(0x[0-9a-f_]+|[A-Z_:a-z]+)", body):
                target = m.group(1)
                if target.startswith("0x"):
                    t = int(target.replace("_", ""), 16)
                else:
                    c = re.search(rf"const {target.split('::')[-1]}: u32 = (0x[0-9a-f_]+);", text) or re.search(rf"const {target.split('::')[-1]}: u32 = (0x[0-9a-f_]+);", "\n".join(p.read_text() for p in GAME.glob('*.rs')))
                    if not c:
                        continue
                    t = int(c.group(1).replace("_", ""), 16)
                if not owners:
                    continue
                if not any(t in calls.get(o, set()) for o in owners):
                    line = text[:start + m.start()].count("\n") + 1
                    print(f"{path.name}:{line}: {name} nests {t:08x}, which " + ", ".join(f"{o:08x}" for o in owners) + " does not call")
                    found += 1
    return 1 if found else 0


if __name__ == "__main__":
    sys.exit(main())
